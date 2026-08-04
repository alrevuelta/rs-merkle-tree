// Copyright 2025 Bilinear Labs - MIT License

//! In-memory store implementation.

use crate::{MerkleError, Node, Store};

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

    fn put(&mut self, level: u32, start: u64, nodes: &[Node]) -> Result<(), MerkleError> {
        if nodes.is_empty() {
            return Ok(());
        }

        let level = level as usize;
        if self.levels.len() <= level {
            self.levels.resize_with(level + 1, Vec::new);
        }

        let stored = &mut self.levels[level];
        let start = usize::try_from(start).unwrap_or(usize::MAX);
        if start > stored.len() {
            return Err(MerkleError::StoreError(format!(
                "non-contiguous put at level {level}: index {start} is past the stored prefix of {}",
                stored.len()
            )));
        }

        // The run may rewrite the tail of the prefix, extend it, or both, and
        // either part is a single copy.
        let overwritten = (stored.len() - start).min(nodes.len());
        stored[start..start + overwritten].copy_from_slice(&nodes[..overwritten]);
        stored.extend_from_slice(&nodes[overwritten..]);

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
        //
        //                  0xff                  <- level 2 (root), index 0
        //                 /    \
        //              0xaa    0xbb              <- level 1, indices 0..1
        //              /  \    /  \
        //           0x0a 0x0b 0x0c 0x0d          <- level 0 (leaves), indices 0..3
        //   index:   0    1    2    3
        store
            .put(0, 0, &[node(0x0a), node(0x0b), node(0x0c), node(0x0d)])
            .unwrap();
        store.put(1, 0, &[node(0xaa), node(0xbb)]).unwrap();
        store.put(2, 0, &[node(0xff)]).unwrap();

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

    #[test]
    fn runs_may_rewrite_the_tail_and_extend_it() {
        let mut store = MemoryStore::new();
        store.put(0, 0, &[node(0x0a), node(0x0b)]).unwrap();

        // The tree rewrites the partial node on the right edge of a level and
        // appends past it in the same run.
        store
            .put(0, 1, &[node(0xbb), node(0x0c), node(0x0d)])
            .unwrap();
        assert_eq!(store.get_num_leaves(), 4);
        assert_eq!(
            store.get(&[0, 0, 0, 0], &[0, 1, 2, 3]).unwrap(),
            vec![
                Some(node(0x0a)),
                Some(node(0xbb)),
                Some(node(0x0c)),
                Some(node(0x0d)),
            ],
        );

        // A run starting past the prefix would leave a hole.
        assert!(store.put(0, 5, &[node(0x0e)]).is_err());
        assert!(store.put(4, 1, &[node(0x0e)]).is_err());
    }
}
