#[derive(Debug, thiserror::Error)]
pub enum MerkleError {
    #[error("Error storing data: {0}")]
    StoreError(String),

    #[error("Leaf index out of bounds: {index}, num_leaves: {num_leaves}")]
    LeafIndexOutOfBounds { index: u64, num_leaves: u64 },

    #[error("Tree is full: depth: {depth}, capacity: {capacity}")]
    TreeFull { depth: u32, capacity: u64 },

    #[error("Levels and indices must have the same length")]
    LengthMismatch { levels: usize, indices: usize },

    #[error("Lock was poisoned: {0}")]
    LockPoisoned(String),
}
