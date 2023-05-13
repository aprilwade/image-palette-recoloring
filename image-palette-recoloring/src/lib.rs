use image::{GenericImageView, ImageBuffer, Luma, Rgb};
use qhull_rs::{ConvexHull, Delaunay};
use nalgebra::{Const, DMatrix, Dyn, Matrix, Vector3, Vector5, Vector6};

mod palette;
mod triangle_distance;

pub use palette::compute_palette;

/// An image represented in terms of the vertices of a 5D RGBXY convex hull.
///
/// This is the first step recoloring an image. Since the process of calculating the per-vertex
/// weights is costly, it is recommended that you keep this data structure around if you plan on
/// creating multiple decompositions of it.
pub struct ImageWeights {
    weights: nalgebra_sparse::CsrMatrix<f64>,
    ch_rgb_vertices: Vec<Vector3<f64>>,
    width: u32,
    height: u32,
}

impl ImageWeights {
    /// Compute the per-vertex weights of the `img`.
    pub fn new(img: &impl GenericImageView<Pixel = Rgb<u8>>) -> Self {
        // We want to represent each 5d-pixel in the image in terms of vertices of the 5d convex
        // hull of all the pixels. To accomplish this, we compute the delaunay triangulation of
        // that convex hull and then Using the triangulation, we find a simplex that contains the
        // pixel and compute the barycentric coordinates.

        let ch: ConvexHull<Const<5>> = img.pixels()
            .map(|(x, y, pix)| [
                    pix[0] as f64 / 255.0,
                    pix[1] as f64 / 255.0,
                    pix[2] as f64 / 255.0,
                    x as f64 / img.width() as f64,
                    y as f64 / img.height() as f64
                ].into())
            .collect();
        let ch_vertices: Vec<_> = ch.vertices()
            .map(|v| v.point())
            .map(|p| [p[0], p[1], p[2], p[3], p[4]])
            .collect();

        // Release the memory of the ConvexHull early
        let _ = ch;

        // Build a triangulation of the convex hull's vertices
        let tri = Delaunay::<Const<5>>::from_arrays(&ch_vertices[..]);

        let vertex_count = ch_vertices.len();
        let row_count = (img.height() * img.width()) as usize;

        // We'll be building a COO matrix, so we need to collect the following vecs
        let mut row_indices = Vec::with_capacity(row_count);
        let mut col_indices = Vec::with_capacity(row_count);
        let mut values = Vec::with_capacity(row_count);

        // For each pixel, find the convex hull that contains the pixel.
        let mut simplex_searcher = tri.simplex_searcher();
        let mut bcoords = Vector6::from_element(0.0);
        // TODO: Look into using rayon to parallelize this computation. Each pixel can be computed
        //       independently with the only minor performance downside.
        for (i, (x, y, pix)) in img.pixels().enumerate() {
            // Here we _must_ find a containing simplex for every pixel. To that end, we start with
            // a relatively tight tolerance which should work for the majority of pixels and then
            // for the pixels that fail, we iteratively loosen the tolerance until we get a match.
            const INITAL_TOLERANCE: f64 = 1e-10;
            simplex_searcher.set_eps(INITAL_TOLERANCE);
            let simplex = loop {
                let simplex = simplex_searcher.find_simplex_mut(
                    &Vector5::new(
                        pix[0] as f64 / 255.0,
                        pix[1] as f64 / 255.0,
                        pix[2] as f64 / 255.0,
                        x as f64 / img.width() as f64,
                        y as f64 / img.height() as f64,
                    ),
                    &mut bcoords,
                );
                if let Some(simplex) = simplex {
                    break simplex
                } else {
                    let current_tolerance = simplex_searcher.eps();
                    simplex_searcher.set_eps(current_tolerance * 2.0);
                }
            };
            for (vert, value) in simplex.vertices().zip(bcoords.as_slice().iter()) {
                row_indices.push(i);
                col_indices.push(vert.index());
                values.push(*value);
            }
        }

        let coo = nalgebra_sparse::CooMatrix::try_from_triplets(
            row_count,
            vertex_count,
            row_indices,
            col_indices,
            values,
        ).unwrap();
        let weights = nalgebra_sparse::CsrMatrix::from(&coo);
        let _ = coo;


        // For a later step, we will also need the RGB submatrix of convex hull, so save that here.
        // Note, we need these to be in the same order as they appear in the triangulation, so we
        // must refer to it's vertices rather than those of the convex hull directly.
        let ch_rgb_vertices = tri.vertices()
            .map(|v| v.point())
            .map(|rgbxy| [rgbxy[0] * 255.0, rgbxy[1] * 255.0, rgbxy[2] * 255.0].into())
            .collect::<Vec<_>>();

        ImageWeights {
            weights,
            ch_rgb_vertices,
            width: img.width(),
            height: img.height(),
        }
    }

    /// The height of the original image
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The width of the original image
    pub fn width(&self) -> u32 {
        self.width
    }
}

/// An image decomposed to a given palette of colors.
///
/// Like with `ImageWeights`, calculating this decomposition expensive and you should avoid doing
/// it repeatedly. By comparison, the act of reconstructing an image is very cheaper.
///
/// It can be useful to immediately reconstruct the image using the decomposition palette as the
/// reconstruction palette. This will give you a sense of how closely the decomposition palette is
/// able to recreate the original image. A "bad" palette will lose some of the information from the
/// original image.
pub struct DecomposedImage {
    matrix: DMatrix<f64>,
    width: u32,
    height: u32,
}

impl DecomposedImage {
    /// Decompose an image into channels based on a palette colors.
    ///
    /// The minimum palette size is 4 colors.
    ///
    /// If your palette has redundant colors, this method will return an error. Whether a color is
    /// redundant is based on the 3D convex hull of the colors in the palette. This can make it
    /// hard to predict which colors will be redundant, however the output of `compute_palette`
    /// should never contain redundant colors.
    pub fn new(img: &ImageWeights, palette: &[Rgb<u8>]) -> Result<Self, String> {
        if palette.len() < 4 {
            return Err(format!(
                "The minimum palette size is 4. Only {} colors were provided.",
                palette.len()
            ))
        }
        let palette_ch: ConvexHull<Const<3>> = palette.iter()
            .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64].into())
            .collect();

        for color in palette {
            let color = [color[0] as f64, color[1] as f64, color[2] as f64];
            if palette_ch.vertices().all(|v| v.point().as_slice() != &color[..]) {
                return Err(
                    "The palette contains redundant colors (not all colors are present in the 3d\
                    convex hull of the palette".into()
                );
            }
        }

        let palette_matrix = crate::palette::compute_star_triangulation_coordinates(
            &palette,
            &palette_ch,
            &img.ch_rgb_vertices[..],
        );

        Ok(DecomposedImage {
            matrix: &img.weights * palette_matrix,
            width: img.width,
            height: img.height,
        })
    }

    /// The number of channels in the palette that was used to create this decomposition.
    pub fn num_channels(&self) -> usize {
        self.matrix.ncols()
    }

    /// The width of the original image
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The height of the original image
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the nth channel of the decomposed image as a grayscale image.
    pub fn get_channel_grayscale(&self, n: usize) -> Option<ImageBuffer<Luma<u8>, Vec<u8>>> {
        if n >= self.matrix.ncols() {
            return None
        }

        let mut vec = Vec::with_capacity(self.matrix.nrows());
        for x in self.matrix.column(n).iter() {
            vec.push((x * 255.0).clamp(0.0, 255.0) as u8);
        }

        Some(ImageBuffer::from_vec(self.width, self.height, vec).unwrap())
    }

    /// Rebuild a recolored image from the new palette.
    ///
    /// Returns None if the provided palette is not the same size as the palette used to build the
    /// decomposed image.
    ///
    /// Compared to creating the image weights and the decomposed image, this is a significantly
    /// cheaper operation.
    pub fn reconstruct(&self, palette: &[Rgb<u8>]) -> Option<ImageBuffer<Rgb<u8>, Vec<u8>>>{
        if palette.len() != self.num_channels() {
            return None
        }
        let palette_matrix = Matrix::<f64, Dyn, Const<3>, _>::from_row_iterator(
            palette.len(),
            palette.iter().flat_map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
        );
        let res = &self.matrix * palette_matrix;

        let mut img = ImageBuffer::from_pixel(self.width, self.height, Rgb([0, 0, 0]));
        for (i, row) in res.row_iter().enumerate() {
            let x = i % self.width as usize;
            let y = i / self.width as usize;
            img.put_pixel(x as u32, y as u32, Rgb([
                row[0].clamp(0.0, 255.0) as u8,
                row[1].clamp(0.0, 255.0) as u8,
                row[2].clamp(0.0, 255.0) as u8,
            ]));
        }
        Some(img)
    }
}

