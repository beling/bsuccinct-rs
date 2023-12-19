use std::ops::BitXorAssign;

use bitm::{BitAccess, BitVec};

pub trait VertexIndex: BitXorAssign + Default + Copy + Sized {
    fn from_usize(u: usize) -> Self;
    fn to_usize(self) -> usize;
}

impl VertexIndex for usize {
    #[inline(always)] fn from_usize(u: usize) -> Self { u }
    #[inline(always)] fn to_usize(self) -> usize { self }
}

impl VertexIndex for u32 {
    #[inline(always)] fn from_usize(u: usize) -> Self { u as u32 }
    #[inline(always)] fn to_usize(self) -> usize { self as usize }
}

/// Packed list of edges incident to the vertex in the 3-regular hyper-graph.
/// 
/// List of *len* edges *(v, a0, b0), (v, a1, b1), ...* incident to the vertex *v*,
/// (where *a0 < b0, a1 < b1, ...*; that is, the edges are in canonical form)
/// is stored as *len, v0, v1*, where *v0 = a0 ^ a1 ^ ...* and *v1 = b0 ^ b1 ^ ...*.
/// 
/// Such representation allows for the easy and fast addition and removal of edges from the list,
/// and the reading of the only edge when *len=1* (see [`XoredAdjacencyList::try_get_edge`]).
/// 
/// For more details, see the paper:
/// - D. Belazzougui, P. Boldi, G. Ottaviano, R. Venturini, S. Vigna, *Cache-Oblivious Peeling of Random Hypergraphs*, 
///   In A. Bilgin, M. W. Marcellin, J. Serra-Sagristà, & J. A. Storer (Eds.),
///   Proceedings of Data Compression Conference 26-28 March 2014, Snowbird,
///   Utah, USA (pp. 352-361). (Data Compression Conference. Proceedings; Vol. 2375-0391).
///   IEEE. <https://doi.org/10.1109/DCC.2014.48>
#[derive(Default, Clone, Copy)]
struct XoredAdjacencyList<VertexIndex = usize> {
    v0: VertexIndex,    // xored first vertices of all incident edges
    v1: VertexIndex,    // xored second vertices of all incident edges
    len: usize    // number of incident edges, if len == 1 than v0 <= v1 are two vertices contained in the only edge
}

impl<VI: VertexIndex> XoredAdjacencyList<VI> {
    /// Adds the vertex to `self`. Must be: `v0 < v1`.
    #[inline] pub fn add_canonized_edge(&mut self, v0: usize, v1: usize) {
        self.v0 ^= VI::from_usize(v0);
        self.v1 ^= VI::from_usize(v1);
        self.len += 1;
    }

    /// Adds the vertex to `self`.
    #[inline] pub fn add_edge(&mut self, v0: usize, v1: usize) {
        if v0 < v1 { self.add_canonized_edge(v0, v1) } else { self.add_canonized_edge(v1, v0) }
    }

    /// Removes the vertex from `self`. Must be: `v0 < v1`.
    #[inline] pub fn remove_canonized_edge(&mut self, v0: usize, v1: usize) {
        self.v0 ^= VI::from_usize(v0);
        self.v1 ^= VI::from_usize(v1);
        self.len -= 1;
    }

    /// Removes the vertex from `self`.
    #[inline] pub fn remove_edge(&mut self, v0: usize, v1: usize) {
        if v0 < v1 { self.remove_canonized_edge(v0, v1) } else { self.remove_canonized_edge(v1, v0) }
    }

    /// Returns the canonized form of the only edge contained in `self` if `self.len==1` or [`None`] otherwise.
    #[inline] pub fn try_get_edge(&mut self) -> Option<(usize, usize)> {
        (self.len == 1).then(|| (self.v0.to_usize(), self.v1.to_usize()))
    }
}

/// The holders of this trait store the values assigned to the edges of the [`HyperGraph`].
pub trait EdgeValues {
    /// Type of stored values.
    type Value;

    /// Add or remove given `value` to/from the list of values assigned to all the given vertices.
    fn add_or_remove_value(&mut self, a: usize, b: usize, c: usize, value: Self::Value);

    /// Returns value assigned to the given `vertex`.
    /// It can only be called when exactly one value is assigned to the edge.
    fn get_value(&self, vertex: usize) -> Self::Value;
}

/// Does not store any values.
impl EdgeValues for () {
    type Value = ();
    #[inline(always)] fn add_or_remove_value(&mut self, _a: usize, _b: usize, _c: usize, _value: Self::Value) {}
    #[inline(always)] fn get_value(&self, _vertex: usize) -> Self::Value {}
}

/// Stores values of a specific (in bits) size.
pub struct PackedInts {
    values: Box<[u64]>,
    bits_per_value: u8
}

impl PackedInts {
    pub fn new(number_of_values: usize, bits_per_value: u8) -> Self {
        Self { values: Box::with_zeroed_bits(number_of_values * bits_per_value as usize), bits_per_value }
    }
}

impl EdgeValues for PackedInts {
    type Value = u64;

    fn add_or_remove_value(&mut self, a: usize, b: usize, c: usize, value: Self::Value) {
        self.values.xor_fragment(a, value, self.bits_per_value);
        self.values.xor_fragment(b, value, self.bits_per_value);
        self.values.xor_fragment(c, value, self.bits_per_value);
    }

    fn get_value(&self, vertex: usize) -> Self::Value {
        self.values.get_fragment(vertex, self.bits_per_value)
    }
}


/// 3-regular hyper-graph.
pub struct HyperGraph<VertexIndex, Values> {
    /// `adjacency_list[v]` is the list of edges incident to the vertex `v`.
    adjacency_list: Vec<XoredAdjacencyList<VertexIndex>>,
    /// Values assigned to the edges.
    values: Values    // values assigned to edges
}

impl<VI: VertexIndex> HyperGraph<VI, ()> {
    /// Constructs hyper-graph with the given `number_of_vertices`.
    #[inline] pub fn new(number_of_vertices: usize) -> Self {
        Self::with_values(number_of_vertices, ())
    }

    /// Adds (`a`, `b`, `c`) edge to `self`.
    #[inline] pub fn add_edge(&mut self, a: usize, b: usize, c: usize) {
        self.add_edge_with_value(a, b, c, ());
    }

    /// Returns a sequence of the graph edges *v0=(a0, b0, c0), v1=(a1, b1, c1), ...*
    /// such that the vertex *ai* is not incident to the edge *vj* for all *j>i*.
    #[inline] pub fn peel(self, number_of_edges: usize) -> Vec<(VI, VI, VI)> {
        self.peel_with_values(number_of_edges, |_| {})
    }
}

impl<VI: VertexIndex> HyperGraph<VI, PackedInts> {
    /// Constructs hyper-graph with the given `number_of_vertices` whose can have assigned values of a given bit-size.
    #[inline] pub fn with_bits_per_value(number_of_vertices: usize, bits_per_value: u8) -> Self {
        Self::with_values(number_of_vertices, PackedInts::new(number_of_vertices, bits_per_value))
    }
}

impl<VI: VertexIndex, Values: EdgeValues> HyperGraph<VI, Values> {
    pub fn with_values(number_of_vertices: usize, values: Values) -> Self {
        Self { adjacency_list: vec![Default::default(); number_of_vertices], values }
    }

    /// Adds to `self` the edge (`a`, `b`, `c`) with assigned `value`.
    pub fn add_edge_with_value(&mut self, a: usize, b: usize, c: usize, value: Values::Value) {
        self.adjacency_list[a].add_edge(b, c);
        self.adjacency_list[b].add_edge(a, c);
        self.adjacency_list[c].add_edge(a, b);
        self.values.add_or_remove_value(a, b, c, value);
    }

    /// If `vertex` is incident with exactly one edge than move this edge from `self` into `vec`,
    /// putting `vertex` at the first position of the `HyperEdge` pushed into vector.
    fn try_move_degree1_vertex_into_vec<VC: FnMut(&Values::Value)>(&mut self, vertex: usize, vec: &mut Vec<(VI, VI, VI)>, value_consumer: &mut VC) {
        if let Some((v0, v1)) = self.adjacency_list[vertex as usize].try_get_edge() {
            self.adjacency_list[vertex as usize].remove_canonized_edge(v0, v1);
            self.adjacency_list[v0 as usize].remove_edge(vertex, v1);
            self.adjacency_list[v1 as usize].remove_edge(v0, vertex);
            vec.push((VI::from_usize(vertex), VI::from_usize(v0), VI::from_usize(v1)));

            let value = self.values.get_value(vertex);
            value_consumer(&value);
            self.values.add_or_remove_value(vertex, v0, v1, value)
        }
    }

    /// Returns a sequence of the graph edges *v0=(a0, b0, c0), v1=(a1, b1, c1), ...*
    /// such that the vertex *ai* is not incident to the edge *vj* for all *j>i*.
    /// Call `value_consumer` for each value assigned to the edge pushed to the returned sequence.
    // Returns [`None`] if a sequence satisfying the above condition could not be found.
    pub fn peel_with_values<VC: FnMut(&Values::Value)>(mut self, number_of_edges: usize, mut value_consumer: VC) -> Vec<(VI, VI, VI)> {
        let mut result = Vec::with_capacity(number_of_edges);
        for vertex in 0..self.adjacency_list.len() {
            self.try_move_degree1_vertex_into_vec(vertex, &mut result, &mut value_consumer);
        }
        let mut i = 0;
        while i < result.len() {
            debug_assert_eq!(self.adjacency_list[result[i].0.to_usize()].len, 0);
            self.try_move_degree1_vertex_into_vec(result[i].1.to_usize(), &mut result, &mut value_consumer);
            self.try_move_degree1_vertex_into_vec(result[i].2.to_usize(), &mut result, &mut value_consumer);
            i += 1;
        }
        result
        //(result.len() == number_of_edges).then(|| (result.into_boxed_slice(), result_values.into_boxed_slice()))
    }
}
