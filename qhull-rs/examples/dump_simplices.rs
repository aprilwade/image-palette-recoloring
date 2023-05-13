use std::convert::TryInto;

use image::io::Reader as ImageReader;
use nalgebra::{Const, OVector, OMatrix};
use qhull_rs::{ConvexHull, Delaunay};

fn main() {
    let img = ImageReader::open("benches/test_image.png").unwrap().decode().unwrap();
    let img = img.into_rgb8();

    let mut vertices: Vec<OVector<f64, Const<5>>> = img.enumerate_pixels()
        .map(|(x, y, pixel)| [
            pixel[0] as f64, pixel[1] as f64, pixel[2] as f64, x as f64, y as f64,
        ].into())
        .collect();
    vertices.sort_by(|a, b| a.as_slice().partial_cmp(&b.as_slice()).unwrap());
    let cv: ConvexHull<_> = vertices.iter().cloned().collect();
    let mut cv_vertices: Vec<[f64; 5]> = cv.vertices()
        .map(|v| v.point().as_slice().try_into().unwrap())
        .collect::<Vec<_>>();
    cv_vertices.sort_by(|a, b| a.as_slice().partial_cmp(&b.as_slice()).unwrap());
    println!("{}", cv_vertices.len());

    let tri = qhull_rs::Delaunay::<nalgebra::U5>::from_arrays(&cv_vertices[..]);
    let _ = cv_vertices;
    println!("{}", tri.simplices().count());

    let mut searcher = tri.simplex_searcher();
    searcher.set_eps(f64::EPSILON * 256.0);
    for vertex in vertices.iter().nth(1) {
        println!("{:?}", vertex.as_slice());
        let mut bcoords = OVector::<f64, Const<6>>::from_element(f64::NAN);
        let simplex = searcher.find_simplex_mut(&vertex, &mut bcoords).unwrap();
        println!("{:?}", simplex.vertices().map(|v| v.point().as_slice()).collect::<Vec<_>>());
    }
}
