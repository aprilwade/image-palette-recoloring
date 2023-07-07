use std::collections::HashMap;

use good_lp::{ProblemVariables, VariableDefinition, SolverModel};
use good_lp::solvers::Solution;
use image::{GenericImageView, Rgb};
use nalgebra::{Const, Vector3, DMatrix, Matrix4, Vector4};
use qhull_rs::{ConvexHull, Delaunay};
use qhull_rs::convex_hull::{Vertex, Facet};

use crate::triangle_distance::triangle_distance_sqr;

// Default error bound: 2.0/255.0

/// Compute a decomposition palette for an image.
///
/// This method computes a palette for an image. The size of the palette is determined by
/// `min_palette_size` (which sets the number of colors to target) and `error_bound` (which sets
/// the amount of per-pixel average error to allow).
///
/// This method computes a palette for an image by representing the pixels as 3D points, computing
/// the convex hull of those points, and then iteratively simplifying the hull until either it
/// reaches `min_palette_size` vertices or the number of average error exceeds `error_bound`. The
/// simplification process tries to always strictly increase the size of the hull. This should
/// prevent any recreation error (since all pixels are still within the polytope), but some
/// vertices might be pushed outside of the 0-255 range, which would create an
/// unrepresentable/"imaginary" color. Thus, those vertices have to be clamped, which can cause
/// some pixels to become unrepresentable.
///
/// Because the simplification process employees 3D polytopes, the smallest a palette can be is 4
/// colors. (After all, the simplest a 3D polytope can be is a tetrahedron.)
///
/// The average error that `error_bound` is compared to is based on the minimum distance between
/// pixels outside the polytope and the nearest facet. This is not a prefect representation of the
/// final reconstruction error that this palette will produce. As a result, you might want to be
/// chose a conservative `error_bound`, or maybe even start with 0. Alternatively, you could set
/// `error_bound` to a very large value and the `min_palette_size` to 4 and then increment
/// `min_palette_size` until the reconstructed image has no visually detectable reconstruction
/// errors.
pub fn compute_palette(
    img: &impl GenericImageView<Pixel = Rgb<u8>>,
    min_palette_size: usize,
    max_palette_size: usize,
    error_bound: f64,
) -> Vec<Rgb<u8>>
{
    // The minimum palette size is 4 because that is the number of vertices of a tetrahedron.
    let min_palette_size = std::cmp::max(min_palette_size, 4);

    let mut ch: ConvexHull<Const<3>> = img.pixels()
        .map(|(_, _, pix)| [
            pix[0] as f64 / 255.0,
            pix[1] as f64 / 255.0,
            pix[2] as f64 / 255.0,
        ].into())
        .collect();
    let mut previous_vcount = ch.vertices().len();

    // Build up the list of unique pixels and their counts
    let mut pixel_map = HashMap::new();
    for (_, _, pixel) in img.pixels() {
        let count = pixel_map.entry(pixel).or_insert(0);
        *count += 1
    }
    let pixel_counts = pixel_map.into_iter()
        .map(|(pixel, count)| (
            Vector3::new(
                pixel[0] as f64 / 255.0,
                pixel[1] as f64 / 255.0,
                pixel[2] as f64 / 255.0,
            ),
            count as f64
        ))
        .collect::<Vec<_>>();
    let total_count: f64 = pixel_counts.iter()
        .map(|(_, count)| *count)
        .sum();

    while ch.vertices().len() > min_palette_size {
        // TODO: We need to calculate the level of error we've created here. This gives us a better
        //       idea of when we should exit the loop
        let (new_vertex, vertices_to_remove) = locate_edge_to_collapse(&ch);
        let new_hull = ch.vertices()
            .filter(|v| !vertices_to_remove.contains(&v.index()))
            .map(|v| v.point())
            .cloned()
            .chain(std::iter::once(new_vertex.into()))
            .collect();

        // Calculating the average error can be expensive, so only do it for the last 6 or so
        // iterations
        if ch.vertices().len() <= max_palette_size {
            let error = compute_pixel_error(&ch, &pixel_counts, total_count);
            if error > error_bound {
                // We've reached or exceeded the error bound, so we are exiting here.
                // We return the previous hull that still was inside the error bound.
                break
            }
        }
        ch = new_hull;

        let vcount = ch.vertices().len();
        if vcount == previous_vcount {
            // If we failed to actually shrink the hull, then we have to bail, unfortunately.
            break
        }
        previous_vcount = vcount;
    }

    ch.vertices()
        .map(|v| v.point())
        .map(|p| Rgb([
            (p[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            (p[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            (p[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        ]))
        .collect()
}

// TODO: Something about how I'm computing the error is wrong or at least doesn't capture the
//       actual reproduction error. :\
fn compute_pixel_error(
    ch: &ConvexHull<Const<3>>,
    pixel_counts: &[(Vector3<f64>, f64)],
    total_count: f64,
) -> f64 {
    let new_ch: ConvexHull<Const<3>> = ch.vertices()
        .map(|v| v.point())
        .map(|p| [p.x.clamp(0.0, 1.0), p.y.clamp(0.0, 1.0), p.z.clamp(0.0, 1.0)].into())
        .collect();
    let tri = Delaunay::from_arrays(ch.vertices()
        .map(|v| v.point())
        .map(|p| [p.x, p.y, p.z])
        .collect::<Vec<[f64; 3]>>()
        .as_slice()
    );
    let mut searcher = tri.simplex_searcher();
    // TODO: Finely tune this value. Too tight of a tolerance will report false positives (errors)
    //       while too loose will report false negatives.
    searcher.set_eps(f64::EPSILON);
    let mut error = 0.0;

    for (pixel, count) in pixel_counts {
        if searcher.find_simplex(pixel).is_some() {
            // This pixel is inside the hull, so we can skip it
            continue
        }
        let minimum_distance = new_ch.facets()
            .map(|f| {
                let mut it = f.vertices();
                let v0 = it.next().unwrap().point();
                let v1 = it.next().unwrap().point();
                let v2 = it.next().unwrap().point();
                triangle_distance_sqr(pixel, v0, v1, v2)
            })
            .min_by(f64::total_cmp)
            .unwrap();
        error += minimum_distance * *count
    }

    // We want the average of the square distance, so divide by the total number of pixels. This
    // way pixels inside the hull pull the error towards 0
    (error / total_count).sqrt()
}




// We're going to simply the convex hull/palette by colapsing edges. It isn't easy to extract edge
// data from qhull, so we'll compute it here in a slightly crude way.
struct EdgeData<'a> {
    edges: Box<[([Vertex<'a, Const<3>>; 2], Box<[Facet<'a, Const<3>>]>)]>,
}

impl<'a> EdgeData<'a> {
    fn new(ch: &'a ConvexHull<Const<3>>) -> Self {
        // TODO: Fuck, we need to build a different kind of hash map too :\
        let mut faces_for_vertex = HashMap::new();
        let mut edges = HashMap::new();
        for facet in ch.facets() {
            let mut it = facet.vertices();
            let a = it.next().unwrap().clone();
            let b = it.next().unwrap().clone();
            let c = it.next().unwrap().clone();
            for v in &[a, b, c] {
                faces_for_vertex.entry(v.clone())
                    .or_insert(vec![])
                    .push(facet.clone())
            }
            for (start, end) in &[(a, b), (b, c), (c, a)] {
                let (start, end) = if start.index() > end.index() {
                    (end, start)
                } else {
                    (start, end)
                };
                edges.entry([*start, *end])
                    .or_insert(vec![]);
                // faces.push(facet)
            }
        }
        for ([start, end], faces) in edges.iter_mut() {
            let start_faces = &faces_for_vertex[start];
            let end_faces = &faces_for_vertex[end];

            // The 2 points should have only 2 faces in common.
            faces.reserve(start_faces.len() + end_faces.len());
            faces.extend(start_faces);
            faces.extend(end_faces);
            faces.sort_by_key(|f: &Facet<Const<3>>| f.index());
            faces.dedup_by_key(|f| f.index());
        }
        let mut edges = edges.into_iter()
            .map(|(key, value)| (key, value.into_boxed_slice()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        edges.sort_by_key(|(verts, _)| (verts[0].index(), verts[1].index()));
        EdgeData {
            edges,
        }
    }

    fn iter<'s>(&'s self)
        -> impl ExactSizeIterator<Item = ([Vertex<'a, Const<3>>; 2], &'s [Facet<'a, Const<3>>])>
    {
        self.edges
            .iter()
            .map(|(vs, fs)| (*vs, &fs[..]))
    }
}


fn locate_edge_to_collapse(ch: &ConvexHull<Const<3>>) -> ([f64; 3], [usize; 2]) {

    let edge_data = EdgeData::new(ch);

    let mut edge_candidates = Vec::with_capacity(edge_data.edges.len());
    for (edge, faces) in edge_data.iter() {


        // Look at the all of the faces to which these 2 vertices are incident. We want to locate a
        // point that will result in positive volume for all of the tetrahedrons formed by each
        // face and the new point. This will ensure that we strictly increase volume when we
        // collapse the edge to this point.
        let mut face_points = Vec::with_capacity(faces.len());
        let mut a = vec![];
        let mut b = vec![];
        let mut c = Vector3::new(0.0, 0.0, 0.0);
        for face in faces {
            // XXX This only works because we ensure a consistent ordering of the vertices of the
            //     facets 3d hulls relative to their normals. QHull does not natively do that for
            //     us :\
            let mut verts = face.vertices();
            let p0 = verts.next().unwrap().point();
            let p1 = verts.next().unwrap().point();
            let p2 = verts.next().unwrap().point();

            face_points.push((p0, p1, p2));

            let mut n = (p1 - p0).cross(&(p2 - p0));
            n.normalize_mut();

            a.push(n);
            b.push(n.dot(&p0));
            c += n;
        }
        for v in &mut a {
            *v *= -1.0;
        }
        for v in &mut b {
            *v *= -1.0;
        }

        let mut vars = ProblemVariables::new();
        let px = vars.add(VariableDefinition::new().name("px"));
        let py = vars.add(VariableDefinition::new().name("py"));
        let pz = vars.add(VariableDefinition::new().name("pz"));
        // Asshole Brits misspelling minimize
        let mut model = vars.minimise(px * c[0] + py * c[1] + pz * c[2])
            .using(good_lp::default_solver);

        for (coefs, bound) in a.iter().zip(&b) {
            model.add_constraint((px * coefs[0] + py * coefs[1] + pz * coefs[2]).leq(*bound));
        }
        match model.solve() {
            Ok(solution) => {
                let p = [solution.value(px), solution.value(py), solution.value(pz)];
                let vol = face_points.iter()
                    .map(|(p0, p1, p2)| tetrahedron_volume(p0, p1, p2, &p.into()))
                    .sum::<f64>();
                edge_candidates.push(([edge[0].index(), edge[1].index()], p, vol));
            }
            Err(_e) => (),
        }
    }
    let (edge_vertices, new_point, _vol) = edge_candidates.iter()
        .min_by(|(_, _, vol_a), (_, _, vol_b)| vol_a.partial_cmp(vol_b).unwrap())
        .cloned()
        .unwrap();

    (new_point, edge_vertices)
}

fn tetrahedron_volume(a: &Vector3<f64>, b: &Vector3<f64>, c: &Vector3<f64>, d: &Vector3<f64>)
    -> f64
{
    (a - d).dot(&(b - d).cross(&(c - d))).abs() / 6.0
}


pub(crate) fn compute_star_triangulation_coordinates(
    palette: &[Rgb<u8>],
    palette_ch: &ConvexHull<Const<3>>,
    img_rgb_values: &[Vector3<f64>],
) -> DMatrix<f64>
{
    // The order of the vertices of the convex hull will (probably) be different the order of the
    // colors in the palette, so we need to build a mapping from the hull vertex indices to the
    // palette indices.
    let palette_ch_vertex_map = palette_ch.vertices()
        .map(|v| v.point())
        .map(|p| Rgb([p[0] as u8, p[1] as u8, p[2] as u8]))
        .map(|p| palette.iter().position(|c| c == &p).unwrap())
        .collect::<Vec<_>>();

    let palette_size = palette_ch.vertices().count();

    // A "star" vertex that will be in every simplex of our triangulation. This should be the color
    // closest to black (and ideally exactly black) to improve the quality of the decomposition.
    let (star_index, star_value) = palette_ch.vertices()
        // Find the palette color closest to zero
        .min_by(|vl, vr| vl.point().norm().total_cmp(&vr.point().norm()))
        .map(|v| (v.index(), v.point().clone()))
        .unwrap();

    // Generate a "star" triangulation. The idea is that the tetrahedrons in the triangulation are
    // constructed using the triangles on the surface of the convex hull plus 1 point in the convex
    // hull - the "star". (Naturally, we exclude the triangles that include the star itself to
    // avoid degenerate tetrahedrons.)

    let solvers: Vec<_> = palette_ch.facets()
        // Facets involving the star vertex would create degenerate tetrahedrons, so skip them.
        .filter(|f| f.vertices().all(|v| v.index() != star_index))
        .map(|f| {
            // TODO: Is there a better way to do this??? Can I do this with an iterator without
            //       completely losing my mind?
            let mut output = Matrix4::from_element(1.0);
            output[0] = star_value[0];
            output[1] = star_value[1];
            output[2] = star_value[2];
            // output[3] = 1.0;
            // for (vertex, col) in f.vertices().zip(output.as_mut_slice()[4..].chunks_mut(4)) {
            for (vertex, mut col) in f.vertices().zip(output.columns_mut(1, 3).column_iter_mut()) {
                let point = vertex.point();
                col[0] = point[0];
                col[1] = point[1];
                col[2] = point[2];
                // col[3] = 1.0;
            }
            output
        })
        // XXX QR or LU? They both should work, I think, but I think QR can handle a greater range
        //     of matrices?
        .map(|mat| mat.lu().try_inverse().unwrap())
        .collect();
    let simplex_indices: Vec<_> = palette_ch.facets()
        .filter(|f| f.vertices().all(|v| v.index() != star_index))
        .map(|f| Vector4::from_iterator(
                std::iter::once(star_index).chain(f.vertices().map(|v| v.index()))
            ))
        .collect();

    // TODO: Should we be using a sparse matrix? Is it likely to matter?
    //       Maybe we could benchmark to get an idea of how it affects performance?
    //       It depends on the number of colors in the palette. The sparcity ratio is exactly the
    //       number of colors / 4.
    let mut matrix = DMatrix::from_element(img_rgb_values.len(), palette_size, 0.0);

    const TOL: f64 = 1e-6;
    for (row_number, pixel) in img_rgb_values.iter().enumerate() {
        let vec = Vector4::new(pixel[0], pixel[1], pixel[2], 1.0);
        let mut bcoords = Vector4::from_element(0.0);
        let matched = solvers.iter()
            .zip(&simplex_indices)
            .find(|(solver, _indices)| {
                solver.mul_to(&vec, &mut bcoords);
                bcoords.iter().all(|p| *p >= -TOL && *p <= 1.0 + TOL)
            });
        let matched_indices = if let Some((_, matched_indices)) = matched {
            matched_indices
        } else {
            // If this point is outside of the convex hull, then we want to find the point on the
            // hull closest to the point.
            let (closest_facet, projected_point, _) = palette_ch.facets()
                .map(|f| {
                    let mut it = f.vertices();
                    let v0 = it.next().unwrap().point().clone();
                    let v1 = it.next().unwrap().point().clone();
                    let v2 = it.next().unwrap().point().clone();
                    let projected_point = crate::triangle_distance::triangle_closest_point(
                        &pixel, &v0, &v1, &v2,
                    );
                    let diff = pixel - projected_point;
                    let dist = diff.dot(&diff);
                    (f, projected_point, dist)
                })
                .min_by(|(_, _, left), (_, _, right)| left.total_cmp(right))
                .unwrap();

            let solver_index = simplex_indices.iter().enumerate()
                .find(|(_, indices)| {
                    // This is a match if the simplex has all 3 of the vertices of closest facet.
                    // There should only every be 1 simplex for which that is true.
                    let indices = indices.as_slice();
                    closest_facet.vertices()
                        .map(|v| v.index())
                        .all(|i| indices.contains(&i))
                })
                .map(|(i, _)| i)
                .unwrap();

            let solver = &solvers[solver_index];
            let vec = Vector4::new(projected_point[0], projected_point[1], projected_point[2], 1.0);
            solver.mul_to(&vec, &mut bcoords);
            &simplex_indices[solver_index]
        };
        let mut row = matrix.row_mut(row_number);
        for (value, index) in bcoords.iter().zip(matched_indices) {
            row[palette_ch_vertex_map[*index]] = *value;
        }
    }

    matrix
}

