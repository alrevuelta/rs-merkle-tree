use crate::node::Node;

use ark_bn254::Fr;

use keccak_batch::{keccak256, keccak256_many};
use light_poseidon::{Poseidon, PoseidonBytesHasher};

pub trait Hasher {
    fn hash(&self, left: &Node, right: &Node) -> Node;

    /// Hashes `pairs[i]` into `out[i]`; `pairs` and `out` must be equally long.
    fn hash_pairs(&self, pairs: &[[Node; 2]], out: &mut [Node]) {
        debug_assert_eq!(pairs.len(), out.len());
        for (parent, [left, right]) in out.iter_mut().zip(pairs) {
            *parent = self.hash(left, right);
        }
    }
}

/// One `left || right` message, the only shape this tree ever hashes.
const MESSAGE: usize = 2 * Node::LEN;

/// Pairs `keccak256_many` hashes per permutation: the four 64-bit SIMD lanes
/// of a 256-bit register, each carrying one independent message.
// TODO: This depends on the hardware. Compilation flag?
const LANES: usize = 4;

pub struct Keccak256Hasher;

impl Hasher for Keccak256Hasher {
    fn hash(&self, left: &Node, right: &Node) -> Node {
        let mut message = [0u8; MESSAGE];
        message[..Node::LEN].copy_from_slice(left.as_ref());
        message[Node::LEN..].copy_from_slice(right.as_ref());
        Node::from(keccak256(message))
    }

    fn hash_pairs(&self, pairs: &[[Node; 2]], out: &mut [Node]) {
        debug_assert_eq!(pairs.len(), out.len());

        // A run of pairs is contiguous bytes, so every pair already sits in
        // memory as the 64-byte message it hashes to; the batch borrows
        // straight from the level instead of staging copies.
        let messages = Node::as_bytes(pairs.as_flattened()).chunks_exact(LANES * MESSAGE);
        let mut parents = out.chunks_exact_mut(LANES);

        for (batch, parents) in messages.zip(parents.by_ref()) {
            let inputs: [&[u8]; LANES] =
                core::array::from_fn(|i| &batch[i * MESSAGE..(i + 1) * MESSAGE]);
            for (parent, digest) in parents.iter_mut().zip(keccak256_many(&inputs)) {
                *parent = Node::from(digest);
            }
        }

        let rest = parents.into_remainder();
        let rest_pairs = &pairs[pairs.len() - rest.len()..];
        for (parent, [left, right]) in rest.iter_mut().zip(rest_pairs) {
            *parent = self.hash(left, right);
        }
    }
}

// Implements the circom-compatible Poseidon hash function (T=3)
pub struct PoseidonHasher;

impl Hasher for PoseidonHasher {
    fn hash(&self, left: &Node, right: &Node) -> Node {
        // circom-compatible Poseidon with 2 inputs (T=3)
        let mut poseidon = Poseidon::<Fr>::new_circom(2).unwrap();

        let res = poseidon
            .hash_bytes_be(&[left.as_ref(), right.as_ref()])
            .unwrap();

        Node::from(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::to_node;

    #[test]
    fn test_keccak256_hash() {
        let hasher = Keccak256Hasher;
        let result = hasher.hash(
            &to_node!("0x1230000000000000000000000000000000000000000000000000000000000000"),
            &to_node!("0x1230000000000000000000000000000000000000000000000000000000000000"),
        );
        assert_eq!(
            result,
            to_node!("0x760bde345debf3075c7fc0bcd2134e16ce5fc1a13adaa66ec6452a391f70595c")
        );
    }

    /// Every batch size around the SIMD lane count, so full batches, the
    /// scalar remainder, and the empty run all reproduce pair-by-pair hashing.
    #[test]
    fn test_keccak256_hash_pairs_matches_hash() {
        let hasher = Keccak256Hasher;
        let pairs: Vec<[Node; 2]> = (0..13).map(|_| [Node::random(), Node::random()]).collect();

        for len in [0, 1, 3, 4, 5, 8, 13] {
            let pairs = &pairs[..len];
            let mut parents = vec![Node::ZERO; len];
            hasher.hash_pairs(pairs, &mut parents);

            for (i, (parent, [left, right])) in parents.iter().zip(pairs).enumerate() {
                assert_eq!(parent, &hasher.hash(left, right), "pair {i} of {len}");
            }
        }
    }

    #[test]
    fn test_poseidon_hash() {
        let hasher = PoseidonHasher;
        let result = hasher.hash(
            &to_node!("0x0000000000000000000000000000000000000000000000000000000000000000"),
            &to_node!("0x0000000000000000000000000000000000000000000000000000000000000000"),
        );

        assert_eq!(
            result,
            to_node!("0x2098f5fb9e239eab3ceac3f27b81e481dc3124d55ffed523a839ee8446b64864")
        );
    }
}
