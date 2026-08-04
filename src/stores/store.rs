// Copyright 2025 Bilinear Labs - MIT License

//! Store module contains the trait definition and some common elements used by the stores.

use crate::{errors::MerkleError, node::Node};

/// Trait that defines a generic API to store and retrieve nodes of a Merkle tree.
/// TODO: Maybe add depth as generic for safety
pub trait Store {
    // TODO: Explore if making this zero copy is worth it.

    /// Returns the nodes at the specified levels and indices. Both lengths
    /// shall have the same size. For example:
    /// levels=[0, 0, 0], indices=[0, 1, 2] to fetch the first 3 indexes at level 0.
    /// The result has the same length as the input levels and indices and a None
    /// item means that the node is not present in the store.
    fn get(&self, levels: &[u32], indices: &[u64]) -> Result<Vec<Option<Node>>, MerkleError>;

    /// Stores `nodes` at `level`, the first at index `start` and the rest
    /// following it, so one call describes one contiguous run.
    fn put(&mut self, level: u32, start: u64, nodes: &[Node]) -> Result<(), MerkleError>;

    /// Returns the number of leaves in the store.
    fn get_num_leaves(&self) -> u64;
}
