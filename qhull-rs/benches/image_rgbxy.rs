// #[macro_use]
// extern crate bencher;

use criterion::{criterion_group, criterion_main, Criterion};

use image::io::Reader as ImageReader;
use qhull_rs::{ConvexHull, Delaunay};

use std::convert::TryInto;

fn find_best_simplex(c: &mut Criterion) {
    let img = ImageReader::open("benches/test_image.png").unwrap().decode().unwrap();
    let img = img.into_rgb8();

    let vertices: Vec<_> = img.enumerate_pixels()
        .map(|(x, y, pixel)| [
            pixel[0] as f64, pixel[1] as f64, pixel[2] as f64, x as f64, y as f64,
        ].into())
        .collect();
    let cv: ConvexHull<_> = vertices.iter().cloned().collect();

    let cv_vertices = cv.vertices().map(|v| v.point().try_into().unwrap()).collect::<Vec<_>>();
    let tri = Delaunay::<nalgebra::U5>::from_arrays(&cv_vertices[..]);
    let _ = cv_vertices;

    let solver_cache = tri.compute_barycentric_solver_cache();
    c.bench_function(
        "find_best_simplex",
        |b| b.iter(|| {
            let mut hint = tri
                .simplices()
                .filter(|s| !s.is_upper_delaunay())
                .next()
                .unwrap();
            for v in vertices.iter() {
                tri.find_best_simplex(v.clone(), &solver_cache, None, None, Some(&mut hint));
            }
        }),
    );
}

// criterion_group!(benches, find_best_simplex);
criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = find_best_simplex
}
criterion_main!(benches);

