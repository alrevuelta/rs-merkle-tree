// Copyright 2025 Bilinear Labs - MIT License

//! In-memory store implementation.

use crate::{MerkleError, Node, Store};
use std::cmp::Ordering;

#[derive(Default)]
pub struct MemoryStore {
    /// `levels[l][idx]` is the node at level `l`, index `idx`. `levels[0]` holds
    /// the leaves, so `levels[0].len()` is the number of leaves.
    levels: Vec<Vec<Node>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Store for MemoryStore {
    fn get(&self, levels: &[u32], indices: &[u64]) -> Result<Vec<Option<Node>>, MerkleError> {
        if levels.len() != indices.len() {
            return Err(MerkleError::LengthMismatch {
                levels: levels.len(),
                indices: indices.len(),
            });
        }

        Ok(levels
            .iter()
            .zip(indices)
            .map(|(&level, &index)| {
                self.levels
                    .get(level as usize)
                    .and_then(|nodes| nodes.get(index as usize))
                    .copied()
            })
            .collect())
    }

    fn put(&mut self, items: &[(u32, u64, Node)]) -> Result<(), MerkleError> {
        let Some(max_level) = items.iter().map(|&(level, _, _)| level).max() else {
            return Ok(());
        };
        let max_level = max_level as usize;
        if self.levels.len() <= max_level {
            self.levels.resize_with(max_level + 1, Vec::new);
        }

        for &(level, index, node) in items {
            let nodes = &mut self.levels[level as usize];
            let index = index as usize;
            match index.cmp(&nodes.len()) {
                Ordering::Equal => nodes.push(node),
                Ordering::Less => nodes[index] = node,
                Ordering::Greater => {
                    return Err(MerkleError::StoreError(format!(
                        "non-contiguous put at level {level}: index {index} is past the stored prefix of {}",
                        nodes.len()
                    )));
                }
            }
        }

        Ok(())
    }

    fn get_num_leaves(&self) -> u64 {
        self.levels.first().map_or(0, |leaves| leaves.len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(byte: u8) -> Node {
        Node::from([byte; Node::LEN])
    }

    #[test]
    fn put_get_roundtrip() {
        let mut store = MemoryStore::new();

        // Simple tree with 4 leaves at the bottom, 2 at level 1, and 1 at level 2.
        store
            .put(&[
                // Merkle tree (4 leaves):
                //
                //                  0xff                  <- level 2 (root), index 0
                //                 /    \
                //              0xaa    0xbb              <- level 1, indices 0..1
                //              /  \    /  \
                //           0x0a 0x0b 0x0c 0x0d          <- level 0 (leaves), indices 0..3
                //   index:   0    1    2    3
                (0, 0, node(0x0a)),
                (0, 1, node(0x0b)),
                (0, 2, node(0x0c)),
                (0, 3, node(0x0d)),
                (1, 0, node(0xaa)),
                (1, 1, node(0xbb)),
                (2, 0, node(0xff)),
            ])
            .unwrap();

        assert_eq!(store.get_num_leaves(), 4);
        assert_eq!(
            {
                let (l, i): (Vec<u32>, Vec<u64>) =
                    [(0, 0), (0, 1), (0, 2), (0, 3), (1, 0), (1, 1), (2, 0)]
                        .into_iter()
                        .unzip();
                store.get(&l, &i).unwrap()
            },
            vec![
                Some(node(0x0a)),
                Some(node(0x0b)),
                Some(node(0x0c)),
                Some(node(0x0d)),
                Some(node(0xaa)),
                Some(node(0xbb)),
                Some(node(0xff)),
            ],
        );
    }
}
