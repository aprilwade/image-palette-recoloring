use std::cell::Cell;
use std::collections::HashMap;

use nalgebra::{OMatrix, OVector, ToTypenum};
use nalgebra::base::allocator::Allocator;
use nalgebra::{Const, DefaultAllocator, DimAdd, DimMin, DimName, DimSum, U1};

use crate::RawDelaunay;

pub type Plus1<N> = DimSum<N, U1>;


const NON_SIMPLEX_IDX: usize = usize::MAX;


/// N dimensional Deluanay triangulation.
///
/// This type stores the data from qhull in a struct-of-arrays format to provide better performance
/// for many operations over the raw qhull structures.
#[derive(Clone)]
pub struct Delaunay<N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, Plus1<N>>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          N: DimName + DimAdd<U1>,
{
    vertices: Box<[OVector<f64, N>]>,
    simplices: Box<[OVector<usize, Plus1<N>>]>,
    neighbors: Box<[OVector<usize, Plus1<N>>]>,
    normals_and_offsets: Box<[(OVector<f64, Plus1<N>>, f64)]>,
    // TODO: There are other things we might want to store, but these will work
    //       for now

    paraboloid_scale: f64,
    paraboloid_shift: f64,
}

impl<N> Delaunay<N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, Plus1<N>>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          N: DimName + DimAdd<U1>,
          DimSum<N, U1>: DimName,
{
    /// Constructs a `Delaunay` from the raw qhull wrapper type.
    ///
    /// This is not the recommended way to construct an instance of this type. You probably want to
    /// use the `from_arrays` method.
    pub fn from_qhull_raw(delaunay: RawDelaunay<N>) -> Self
        where DefaultAllocator: Allocator<f64, nalgebra::DimMinimum<Plus1<N>, Plus1<N>>>,
              DefaultAllocator: Allocator<(usize, usize), Plus1<N>>,
              Plus1<N>: nalgebra::DimMin<Plus1<N>, Output = Plus1<N>>,
    {
        // TODO: I think we can get a size hint from qhull somehow
        let mut vertices = vec![];
        let mut vertex_id_map = HashMap::<u64, usize>::new();
        let mut simplices = vec![];
        let mut simplex_id_map = HashMap::<u64, usize>::new();
        let mut normals_and_offsets = vec![];

        for simplex in delaunay.simplices() {
            if simplex.is_upper_delaunay() {
                continue
            }
            let next_id = simplex_id_map.len();
            simplex_id_map.insert(simplex.id(), next_id);
            normals_and_offsets.push((
                OVector::<f64, Plus1<N>>::from_row_slice(simplex.normal()),
                simplex.offset(),
            ));
            let it = simplex.vertices().map(|vertex| {
                *vertex_id_map.entry(vertex.id())
                    .or_insert_with(|| {
                        // We only want to capture vertices that are present in simplices that
                        // aren't upper delaunay
                        let idx = vertices.len();
                        vertices.push(OVector::<f64, N>::from_row_slice(vertex.point()));
                        idx
                    })
            });
            simplices.push(OVector::<usize, Plus1<N>>::from_iterator(it));
        }

        let mut neighbors = Vec::with_capacity(simplices.len());
        for simplex in delaunay.simplices() {
            if simplex.is_upper_delaunay() {
                continue
            }
            neighbors.push(OVector::<usize, Plus1<N>>::from_iterator(simplex.neighbors()
                .map(|s| {
                    if s.is_upper_delaunay() {
                        NON_SIMPLEX_IDX
                    } else {
                        simplex_id_map[&s.id()]
                    }
                })
            ));
        }

        let last_low = delaunay.last_low();
        let last_high = delaunay.last_high();
        let last_newhigh = delaunay.last_newhigh();

        let paraboloid_scale = last_newhigh / last_high - last_low;

        Delaunay {
            vertices: vertices.into_boxed_slice(),
            simplices: simplices.into_boxed_slice(),
            neighbors: neighbors.into_boxed_slice(),
            normals_and_offsets: normals_and_offsets.into_boxed_slice(),

            paraboloid_scale,
            paraboloid_shift: last_low * paraboloid_scale,
        }
    }

    /// Iterator over the vertices of the triangulation.
    pub fn vertices<'a>(&'a self) -> impl ExactSizeIterator<Item = Vertex<'a, N>> + 'a {
        (0..self.vertices.len()).map(move |i| Vertex { delaunay: self, idx: i })
    }

    /// Iterator over the vertices of the simplices.
    pub fn simplices<'a>(&'a self) -> impl ExactSizeIterator<Item = Simplex<'a, N>> + 'a {
        (0..self.simplices.len()).map(move |i| Simplex { delaunay: self, idx: i })
    }

    pub fn simplex_searcher(&'_ self) -> SimplexSearcher<'_, N>
        where DefaultAllocator: Allocator<f64, N, N>,
              DefaultAllocator: Allocator<(usize, usize), N>,
              N: DimMin<N, Output = N>,
    {
        SimplexSearcher::new(self)
    }

    /// Adds an N+1 dimension to an N dimension vector
    ///
    /// The Delaunay triangulation is calculated by lifting N dimensional points into N+1
    /// dimensions and then computing a convex hull. This method performs the lifting operation for
    /// an arbitrary point. This is necessary for searching for a containing simplex for a point.
    fn lift_point(&self, point: &OVector<f64, N>) -> OVector<f64, Plus1<N>> {
        let ndim = N::dim();
        let mut output = OVector::<f64, Plus1<N>>::from_element(0.0);
        for i in 0..ndim {
            output[i] = point[i];
            output[ndim] += (output[i]).powi(2);
        }
        output[ndim] *= self.paraboloid_scale;
        output[ndim] += self.paraboloid_shift;
        output
    }
}

impl<const N: usize> Delaunay<Const<N>>
    where DefaultAllocator: Allocator<f64, Const<N>>,
          DefaultAllocator: Allocator<usize, Const<N>>,
          DefaultAllocator: Allocator<usize, Plus1<Const<N>>>,
          DefaultAllocator: Allocator<f64, Plus1<Const<N>>, Plus1<Const<N>>>,
          Const<N>: ToTypenum + DimName + DimAdd<U1>,
          Plus1<Const<N>>: DimName,
          DefaultAllocator: Allocator<f64, nalgebra::DimMinimum<Plus1<Const<N>>, Plus1<Const<N>>>>,
          DefaultAllocator: Allocator<(usize, usize), Plus1<Const<N>>>,
          Plus1<Const<N>>: nalgebra::DimMin<Plus1<Const<N>>, Output = Plus1<Const<N>>>,
{
    pub fn from_arrays(slice: &[[f64; N]]) -> Self {
        Self::from_qhull_raw(RawDelaunay::from_arrays(slice))
    }
}

/// A vertex of a Delaunay triangulation.
#[derive(Copy, Clone)]
pub struct Vertex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, Plus1<N>>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          N: DimName + DimAdd<U1>,
{
    delaunay: &'a Delaunay<N>,
    idx: usize,
}

impl<'a, N> Vertex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, Plus1<N>>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          N: DimName + DimAdd<U1>,
{
    /// The N dimensional point at which this vertex is located.
    pub fn point(&self) -> &'a OVector<f64, N> {
        self.delaunay.vertices.get(self.idx).unwrap()
    }

    /// The index of this vertex inside the struct-of-arrays of the `ConvexHull`. This is mostly
    /// useful if you want to associate extra data with this vertex in a side table and don't want
    /// to use the vertex itself as the key in a map.
    pub fn index(&self) -> usize {
        self.idx
    }
}

/// A simplex of a Delaunay triangulation.
#[derive(Copy, Clone)]
pub struct Simplex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, Plus1<N>>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          N: DimName + DimAdd<U1>,
{
    delaunay: &'a Delaunay<N>,
    idx: usize,
}

impl<'a, N> Simplex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, Plus1<N>>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          N: DimName + DimAdd<U1>,
{
    /// Iterator over the vertices that make up this simplex.
    pub fn vertices(&self) -> impl ExactSizeIterator<Item = Vertex<'a, N>> + 'a {
        let delaunay = self.delaunay;
        delaunay.simplices.get(self.idx).unwrap().iter()
            .map(move |&idx| Vertex { delaunay, idx })
    }

    /// Iterator over the neighboring simplices of this simplex.
    pub fn neighbors(&self) -> impl ExactSizeIterator<Item = Option<Simplex<'a, N>>> + 'a {
        let delaunay = self.delaunay;
        delaunay.neighbors.get(self.idx).unwrap().iter()
            .map(move |&idx| if idx == NON_SIMPLEX_IDX {
                    None
                } else {
                    Some(Simplex { delaunay, idx: idx })
                })

    }

    /// The plane distance between an N+1 dimension point and this simpelx.
    pub fn plane_distance(&self, point: &OVector<f64, Plus1<N>>) -> f64 {
        let (normal, offset) = self.delaunay.normals_and_offsets.get(self.idx).unwrap();
        let mut sum = *offset;
        for (a, b) in point.as_slice().iter().zip(normal.as_slice()) {
            sum += a + b;
        }
        sum
    }

    /// The index of this simplex inside the struct-of-arrays of the `ConvexHull`. This is mostly
    /// useful if you want to associate extra data with this simplex in a side table and don't want
    /// to use the simpelx itself as the key in a map.
    pub fn index(&self) -> usize {
        self.idx
    }
}


/// Helper for locating which simplex contains a given point.
///
/// This type exists to hold the settings that could influence how the search could be performed
/// and to keep track of the where the previous search ended in order to improve the performance of
/// subsequence searchers by starting from a location that is hopefully near-by. Placing this in a
/// struct instead of on a method in `Delaunay` makes for an easier to use API.
///
/// The algorithm used here is heavily inspired by the implementation in SciPy.
pub struct SimplexSearcher<'a, N >
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, Plus1<N>>,
          DefaultAllocator: Allocator<f64, N, N>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          N: DimName + DimAdd<U1>,
{
    delaunay: &'a Delaunay<N>,
    transforms: Box<[BarycentricTransformation<N>]>,

    max_bounds: OVector<f64, N>,
    min_bounds: OVector<f64, N>,

    bruteforce: bool,
    eps: f64,

    start_hint: Cell<Option<usize>>,
}

impl<'a, N> SimplexSearcher<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, Plus1<N>>,
          DefaultAllocator: Allocator<f64, N, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<(usize, usize), N>,
          N: DimName + DimAdd<U1>,
          DimSum<N, U1>: DimName,
{
    pub fn new(delaunay: &'a Delaunay<N>) -> Self
        where N: DimMin<N, Output = N>,
    {
        let ndim = N::dim();
        let cmp = |a: &f64, b: &f64| a.partial_cmp(b).unwrap();
        let max_bounds = OVector::<f64, N>::from_iterator(
            (0..ndim).map(|i| delaunay.vertices().map(|v| v.point()[i]).max_by(cmp).unwrap())
        );
        let min_bounds = OVector::<f64, N>::from_iterator(
            (0..ndim).map(|i| delaunay.vertices().map(|v| v.point()[i]).min_by(cmp).unwrap())
        );
        let transforms = delaunay.simplices()
            .map(|s| BarycentricTransformation::new(s))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        SimplexSearcher {
            delaunay,
            transforms,

            max_bounds,
            min_bounds,

            bruteforce: false,
            eps: f64::EPSILON,

            start_hint: Cell::new(None),
        }
    }

    /// Returns `true` if the searcher will always employ bruteforce search instead of just using
    /// bruteforce as a fallback.
    ///
    /// Default value is `false`.
    pub fn bruteforce(&self) -> bool {
        self.bruteforce
    }

    /// Set whether or not a bruteforce approach should always be used instead as just a fallback
    /// for when the directed search fails.
    ///
    /// Default value is `false`.
    pub fn set_bruteforce(&mut self, bruteforce: bool) -> &mut Self {
        self.bruteforce = bruteforce;
        self
    }

    /// The tolerance for determining if a point lies within a simplex.
    ///
    /// The search process calculates the barycentric coordinates of the target point within the
    /// simplices of the triangulation. If all the barycentric coordinates fallwithin -eps and 1 +
    /// eps, then the point is considered to be inside the simplex and the search stops.
    ///
    /// The default is `f64::EPSILON`.
    pub fn eps(&self) -> f64 {
        self.eps
    }

    /// The tolerance for determining if a point lies within a simplex.
    ///
    /// The search process calculates the barycentric coordinates of the target point within the
    /// simplices of the triangulation. If all the barycentric coordinates fallwithin -eps and 1 +
    /// eps, then the point is considered to be inside the simplex and the search stops.
    ///
    /// The default is `f64::EPSILON`.
    pub fn set_eps(&mut self, eps: f64) -> &mut Self {
        self.eps = eps;
        self
    }

    /// Removes the previous
    pub fn clear_start_hint(&mut self) -> &mut Self {
        self.start_hint.set(None);
        self
    }

    /// Attempts to locates a simplex within the triangulation that contains `point`.
    ///
    /// Returns None if the search fails to find a containing simplex.
    pub fn find_simplex(&self, point: &OVector<f64, N>) -> Option<Simplex<'a, N>>
        where DefaultAllocator: Allocator<f64, Plus1<N>>,
    {
        let mut coords = OVector::<f64, Plus1<N>>::from_element(0.0);
        self.find_simplex_mut(point, &mut coords)
    }

    /// Attempts to locates a simplex within the triangulation that contains `point` and saves in
    /// `coords` the computed barycentric for the final, matching simplex.
    ///
    /// Returns None if the search fails to find a containing simplex. Likewise, if the search
    /// fails, the contents of `coords` should be assumed to be junk.
    pub fn find_simplex_mut(
        &self,
        point: &OVector<f64, N>,
        coords: &mut OVector<f64, Plus1<N>>
    ) -> Option<Simplex<'a, N>>
        where DefaultAllocator: Allocator<f64, Plus1<N>>,
    {
        if self.bruteforce {
            return self.find_best_simplex_bruteforce(point, coords)
        }


        let start_simplex_idx = self.start_hint.get().unwrap_or(1);
        let mut simplex = Simplex { delaunay: self.delaunay, idx: start_simplex_idx };

        let lifted_point = self.delaunay.lift_point(point);
        let mut best_distance = simplex.plane_distance(&lifted_point);
        let mut changed = true;
        while changed && best_distance <= 0.0 {
            changed = false;

            for neighbor in simplex.neighbors() {
                let Some(neighbor) = neighbor else {
                    continue
                };
                let dist = neighbor.plane_distance(&lifted_point);
                if dist > best_distance + self.eps * (1.0 + best_distance.abs()) {
                    simplex = neighbor;
                    best_distance = dist;
                    changed = true;
                }
            }
        }

        self.find_best_simplex_directed(point, simplex, coords)
    }

    fn find_best_simplex_bruteforce(
        &self,
        point: &OVector<f64, N>,
        bcoords: &mut OVector<f64, Plus1<N>>
    ) -> Option<Simplex<'a, N>>
        where DefaultAllocator: Allocator<f64, Plus1<N>>,
    {
        if point.iter().zip(self.max_bounds.as_slice()).any(|(x, bound)| x > &(bound + self.eps)) {
            return None
        }
        if point.iter().zip(self.min_bounds.as_slice()).any(|(x, bound)| x < &(bound - self.eps)) {
            return None
        }

        let eps_broad = self.eps.sqrt();

        for i in 0..self.delaunay.simplices.len() {

            let transform = self.transforms.get(i).unwrap();
            if !transform.is_degenerate() {
                transform.solve(point.clone(), bcoords);

                if bcoords.iter().all(|f| *f >= -self.eps && *f <= 1.0 + self.eps) {
                    return Some(Simplex { delaunay: self.delaunay, idx: i })
                }
            } else {
                // We have a degenerate simplex (or at the very least couldn't generate an
                // inverse matrix to compute barycentric coordinates). Check each neighbor with
                // a larger-than-normal tolerance.
                for &neighbor in &self.delaunay.neighbors[i] {
                    if neighbor == NON_SIMPLEX_IDX {
                        continue
                    }
                    let transform = self.transforms.get(neighbor).unwrap();
                    if transform.is_degenerate() {
                        // This neighbor is degenerate too, skip it
                        continue
                    }
                    let inside = self.delaunay.neighbors[neighbor].iter()
                        .zip(bcoords.iter())
                        .filter(|(i, _)| **i != NON_SIMPLEX_IDX)
                        .all(|(inner_neighbor, coord)| {
                            if inner_neighbor == &i {
                                *coord >= -eps_broad && *coord <= 1.0 + self.eps
                            } else {
                                *coord >= -self.eps && *coord <= 1.0 + self.eps
                            }
                        });
                    if inside {
                        return Some(Simplex { delaunay: self.delaunay, idx: neighbor })
                    }
                }
            }
        }
        None
    }

    fn find_best_simplex_directed(
        &self,
        point: &OVector<f64, N>,
        start: Simplex<'a, N>,
        bcoords: &mut OVector<f64, Plus1<N>>
    ) -> Option<Simplex<'a, N>>
        where DefaultAllocator: Allocator<f64, Plus1<N>>,
    {
        let mut simplex = start;
        'outer: for _cycle in 0..(self.delaunay.simplices.len() / 4 + 1) {
            let transform = self.transforms.get(simplex.idx).unwrap();

            if transform.is_degenerate() {
                // If we encounter a degenerate simplex, fall back to bruteforce
                break
            }

            let mut inside = true;
            // Compute each barycentric coordinate one at a time
            // (The iterator both populates the output vector and yields the coords.)
            let iter = simplex.neighbors().zip(transform.iter_bcoords(point.clone(), bcoords));
            for (neighbor, bcoord) in iter {
                if bcoord < -self.eps {
                    // Find the neighbor corresponding to the coord
                    if let Some(neighbor) = neighbor {
                        // The neighbor exists (isn't outer-delaunay) so, retry starting from
                        // it.
                        simplex = neighbor;
                        // break
                        continue 'outer;
                    } else {
                        // This suggests the point lives outside of the triangulation, so we're
                        // supposed to bail out here.
                        // TODO: We might want to force a bruteforce instead since we have
                        //       numerical stability issues compared to scipy's implementation.
                        self.start_hint.set(Some(simplex.idx));
                        return None

                        // break 'outer // force the bruteforce instead
                    }
                } else if !(bcoord <= 1.0 + self.eps) {
                    // Keep looking for a negative coordinate but note this isn't a match
                    inside = false;
                }/* else {
                     //This is what we actually want to see.
                }*/

            }

            if inside {
                // Success!

                // Remember this simplex as a start point for next time
                self.start_hint.set(Some(simplex.idx));

                return Some(simplex)
            }

            // We weren't inside the simplex and we didn't find a negative barycentric coordinate,
            // so we must have encountered a degenerate simplex. Fallback to bruteforce.
            break;
        }
        // Fallback to bruteforce
        let result = self.find_best_simplex_bruteforce(point, bcoords);
        if let Some(result) = &result {
            self.start_hint.set(Some(result.idx))
        }
        result
    }

    pub fn barycentric_coords(
        &self,
        simplex: Simplex<'a, N>,
        point: OVector<f64, N>,
    ) -> OVector<f64, Plus1<N>>
        where DefaultAllocator: Allocator<f64, Plus1<N>>,
    {
        assert!(std::ptr::eq(simplex.delaunay, self.delaunay));
        let mut coords = OVector::<f64, Plus1<N>>::from_element(0.0);
        self.barycentric_coords_mut(simplex, point, &mut coords);
        coords
    }

    pub fn barycentric_coords_mut(
        &self,
        simplex: Simplex<'a, N>,
        point: OVector<f64, N>,
        output: &mut OVector<f64, Plus1<N>>,
    )
        where DefaultAllocator: Allocator<f64, Plus1<N>>,
    {
        assert!(std::ptr::eq(simplex.delaunay, self.delaunay));
        self.transforms.get(simplex.idx).unwrap().solve(point, output);
    }
}


struct BarycentricTransformation<N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<f64, N, N>,
          N: DimName,
{
    t_inv: OMatrix<f64, N, N>,
    r_vec: OVector<f64, N>,
}

impl<N> BarycentricTransformation<N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          DefaultAllocator: Allocator<f64, N, N>,
          DefaultAllocator: Allocator<usize, Plus1<N>>,
          N: DimName + DimAdd<U1>,
{
    fn new(simplex: Simplex<'_, N>) -> Self
        where DefaultAllocator: Allocator<f64, Plus1<N>>,
              DefaultAllocator: Allocator<(usize, usize), <N as DimMin<N>>::Output>,
              DefaultAllocator: Allocator<(usize, usize), N>,
              N: DimMin<N, Output = N>,
    {
        let ndim = N::dim();
        let delaunay = simplex.delaunay;
        let vertex_indices = delaunay.simplices.get(simplex.idx).unwrap();
        let last_row_idx = *vertex_indices.get(ndim).unwrap();
        let last_row = delaunay.vertices.get(last_row_idx).unwrap();
        let iter = last_row.iter()
            .enumerate()
            .flat_map(|(i, rn)| vertex_indices.as_slice()[..ndim].iter()
                .map(move |vert_idx| delaunay.vertices.get(*vert_idx).unwrap())
                .map(move |vert| vert[i] - rn)
            );
        let t = OMatrix::<f64, N, N>::from_row_iterator(iter);
        let decomp = t.lu();
        let mut t_inv = OMatrix::<f64, N, N>::from_element(f64::NAN);
        let _ = decomp.try_inverse_to(&mut t_inv);

        BarycentricTransformation {
            t_inv,
            r_vec: last_row.clone(),
        }
    }

    fn is_degenerate(&self) -> bool {
        self.t_inv[(0, 0)].is_nan()
    }

    fn solve(&self, point: OVector<f64, N>, output: &mut OVector<f64, DimSum<N, U1>>)
        where DefaultAllocator: Allocator<f64, DimSum<N, U1>>,
              N: DimAdd<U1>,
    {
        let ndim = N::dim();
        output[ndim] = 1.0;
        // TODO: Can I just do this as a matrix multipication?
        for i in 0..ndim {
            output[i] = 0.0;
            for j in 0..ndim {
                output[i] += self.t_inv[(i, j)] * (point[j] - self.r_vec[j]);
            }
            output[ndim] -= output[i];
        }
    }

    fn iter_bcoords<'a>(
        &'a self,
        point: OVector<f64, N>,
        output: &'a mut OVector<f64, Plus1<N>>,
    ) -> impl ExactSizeIterator<Item = f64> + 'a
        where DefaultAllocator: Allocator<f64, DimSum<N, U1>>,
              N: DimAdd<U1>,
    {
        let ndim = N::dim();
        output[ndim] = 1.0;
        (0..(ndim + 1))
            .map(move |i| {
                if i < ndim {
                    output[i] = 0.0;
                    for j in 0..ndim {
                        output[i] += self.t_inv[(i, j)] * (point[j] - self.r_vec[j]);
                    }
                    output[ndim] -= output[i];
                }
                output[i]
            })
    }
}



#[test]
fn test_construction() {
    let points = [
        [1.0, 1.0],
        [2.0, 1.0],
        [1.0, 2.0],
        [2.0, 2.0],
    ];
    let tri = Delaunay::from_arrays(&points[..]);

    let mut simplices = tri.simplices()
        .map(|s| {
            let mut points = s.vertices()
                .map(|v| v.point().as_slice())
                .collect::<Vec<_>>();
            points.sort_by(|a, b| a.partial_cmp(b).unwrap());
            points
        })
        .collect::<Vec<_>>();
    simplices.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(simplices, [
        [[1.0, 1.0], [1.0, 2.0], [2.0, 2.0]],
        [[1.0, 1.0], [2.0, 1.0], [2.0, 2.0]],
    ][..]);
}
