use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::iter::FromIterator;

use nalgebra::{OVector, ToTypenum};
use nalgebra::base::allocator::Allocator;
use nalgebra::{Const, DefaultAllocator, DimName};

use crate::RawHull;

/// N dimensional convex hull
///
/// This type stores the data from qhull in a struct-of-arrays format to provide better performance
/// for many operations over the raw qhull structures.
#[derive(Clone, Debug)]
pub struct ConvexHull<N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    vertices: Box<[OVector<f64, N>]>,
    facets: Box<[OVector<usize, N>]>,
    neighbors: Box<[OVector<usize, N>]>,
    offsets_and_normals: Box<[(OVector<f64, N>, f64)]>,
    // TODO: There are other things we might want to store, but these will work
    //       for now. Example: facets that neighbor a vertex
}

impl<N> ConvexHull<N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    /// Constructs a `ConvexHull` from the raw qhull wrapper type.
    ///
    /// This is not the recommended way to construct an instance of this type. You probably want to
    /// use `.collect()` or the `from_arrays` method.
    pub fn from_qhull_raw(hull: RawHull<N>) -> Self
    {
        let mut vertices = vec![];
        // TODO: This could be a vec itself. not clear if that's faster though
        //       Likewise, we could figure out the capacity of these before allocating them
        let mut vertex_id_map = HashMap::<u64, usize>::new();
        for vertex in hull.vertices() {
            assert!(vertex_id_map.insert(vertex.id(), vertices.len()).is_none());
            vertices.push(OVector::<f64, N>::from_row_slice(vertex.point()));
        }
        let mut facets = vec![];
        let mut offsets_and_normals = vec![];
        let mut facet_id_map = HashMap::<u64, usize>::new();
        for facet in hull.facets() {
            facet_id_map.insert(facet.id(), facets.len());
            offsets_and_normals.push((
                // XXX This feels really gross, but it's necessary until a change is made to the
                //     facet api :(
                OVector::<f64, N>::from_row_slice(&facet.normal()[..N::USIZE]),
                facet.offset(),
            ));
        }
        for facet in hull.facets() {
            let it = facet.vertices().map(|v| vertex_id_map[&v.id()]);
            facets.push(OVector::<usize, N>::from_iterator(it));
        }
        let mut neighbors: Vec<_> = hull.facets()
            .map(|f| {
                let it = f.neighbors().map(|f| facet_id_map[&f.id()]);
                OVector::<usize, N>::from_iterator(it)
            })
            .collect();
        if N::USIZE == 3 {
            for i in 0..facets.len() {
                let p0 = &vertices[facets[i][0]];
                let p1 = &vertices[facets[i][1]];
                let p2 = &vertices[facets[i][2]];
                let n = (p1 - p0).cross(&(p2 - p0));
                let normal = &offsets_and_normals[i].0;
                if normal.dot(&n) < 0.0 {
                    // We need to swap the order of some vertices so they are consistent relative
                    // to the normal for the face. I feel like it would be best if qhull handled
                    // this for us, but who the fuck am I.
                    let (v0, rest) = facets[i].as_mut_slice().split_first_mut().unwrap();
                    let (v1, _) = rest.split_first_mut().unwrap();
                    std::mem::swap(v0, v1);
                    let (n0, rest) = neighbors[i].as_mut_slice().split_first_mut().unwrap();
                    let (n1, _) = rest.split_first_mut().unwrap();
                    std::mem::swap(n0, n1);

                }
            }
        }

        ConvexHull {
            vertices: vertices.into_boxed_slice(),
            facets: facets.into_boxed_slice(),
            neighbors: neighbors.into_boxed_slice(),
            offsets_and_normals: offsets_and_normals.into_boxed_slice(),
        }
    }

    // TODO: Create full iterator types for this.
    /// Iterator over all vertices of the convex hull.
    pub fn vertices<'a>(&'a self) -> impl ExactSizeIterator<Item = Vertex<'a, N>> + 'a {
        (0..self.vertices.len()).map(move |idx| Vertex { hull: self, idx })
    }

    /// Iterator over all facets of the convex hull.
    pub fn facets<'a>(&'a self) -> impl ExactSizeIterator<Item = Facet<'a, N>> + 'a {
        (0..self.facets.len()).map(move |idx| Facet { hull: self, idx })
    }
}

impl<const N: usize> ConvexHull<Const<N>>
    where DefaultAllocator: Allocator<f64, Const<N>>,
          DefaultAllocator: Allocator<usize, Const<N>>,
          Const<N>: ToTypenum,
{
    pub fn from_arrays(slice: &[[f64; N]]) -> Self {
        Self::from_qhull_raw(RawHull::from_arrays(slice))
    }
}

impl<N> FromIterator<OVector<f64, N>> for ConvexHull<N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    fn from_iter<T: IntoIterator<Item = OVector<f64, N>>>(iter: T) -> Self {
        Self::from_qhull_raw(RawHull::from_iter(iter))
    }
}

/// Vertex of an N dimensional convex hull.
#[derive(Clone, Copy, Debug)]
pub struct Vertex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    hull: &'a ConvexHull<N>,
    idx: usize,
}

impl<'a, N> Vertex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    /// The N dimensional point at which this vertex is located.
    pub fn point(&self) -> &'a OVector<f64, N> {
        self.hull.vertices.get(self.idx).unwrap()
    }

    /// The index of this vertex inside the struct-of-arrays of the `ConvexHull`. This is mostly
    /// useful if you want to associate extra data with this vertex in a side table and don't want
    /// to use the vertex itself as the key in a map.
    pub fn index(&self) -> usize {
        self.idx
    }
}

impl<'a, N> Hash for Vertex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    fn hash<H>(&self, state: &mut H)
       where H: Hasher
    {
        (self.hull as *const ConvexHull<N>).hash(state);
        self.idx.hash(state);
    }
}

impl<'a, N> PartialEq for Vertex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    fn eq(&self, other: &Self) -> bool {
        (self.hull as *const _) == (other.hull as *const _) && self.idx == other.idx
    }
}

impl<'a, N> Eq for Vertex<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName
{ }


/// Facet of an N dimensional convex hull.
#[derive(Clone, Copy, Debug)]
pub struct Facet<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    hull: &'a ConvexHull<N>,
    idx: usize,
}

impl<'a, N> Facet<'a, N>
    where DefaultAllocator: Allocator<f64, N>,
          DefaultAllocator: Allocator<usize, N>,
          N: DimName,
{
    /// Iterator over the vertices of this facet.
    pub fn vertices(&self) -> impl ExactSizeIterator<Item = Vertex<'a, N>> + 'a {
        let hull = self.hull;
        hull.facets.get(self.idx).unwrap().iter()
            .map(move |&idx| Vertex { hull, idx })
    }

    /// Iterator over the neighboring facets of this facet.
    pub fn neighbors(&self) -> impl ExactSizeIterator<Item = Facet<'a, N>>+ 'a {
        let hull = self.hull;
        hull.neighbors.get(self.idx).unwrap().iter()
            .map(move |&idx| Facet { hull, idx })
    }

    /// The normal vector of this facet.
    pub fn normal(&self) -> &'a OVector<f64, N> {
        &self.hull.offsets_and_normals.get(self.idx).unwrap().0
    }

    pub fn offset(&self) -> f64 {
        self.hull.offsets_and_normals.get(self.idx).unwrap().1
    }

    /// The index of this facet inside the struct-of-arrays of the `ConvexHull`. This is mostly
    /// useful if you want to associate extra data with this facet in a side table and don't want
    /// to use the facet itself as the key in a map.
    pub fn index(&self) -> usize {
        self.idx
    }
}




// Yes, this is the same test as raw::test_cv_construction. That's not really an accident.
#[test]
fn test_construction() {
    let points = [
        [1.0, 1.0],
        [2.0, 1.0],
        [1.0, 2.0],
        [2.0, 2.0],
        [1.5, 1.5],
        [1.5, 1.0],
    ];
    let cv = ConvexHull::from_arrays(&points[..]);
    let mut vertices = cv.vertices().map(|v| v.point().as_slice()).collect::<Vec<_>>();
    vertices.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(vertices, [
        [1.0, 1.0],
        [1.0, 2.0],
        [2.0, 1.0],
        [2.0, 2.0],
    ][..]);

    let mut facets = cv.facets()
        .map(|f| {
            let mut points = f.vertices()
                .map(|v| v.point().as_slice())
                .collect::<Vec<_>>();
            points.sort_by(|a, b| a.partial_cmp(b).unwrap());
            points
        })
        .collect::<Vec<_>>();
    facets.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(facets, [
        [[1.0, 1.0], [1.0, 2.0]],
        [[1.0, 1.0], [2.0, 1.0]],
        [[1.0, 2.0], [2.0, 2.0]],
        [[2.0, 1.0], [2.0, 2.0]],
    ][..]);
}
