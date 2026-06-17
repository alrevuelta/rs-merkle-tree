// Copyright 2025 Bilinear Labs - MIT License

//! Merkle tree implementation.

use crate::hasher::{Hasher, Keccak256Hasher};
use crate::{MerkleError, Node, Store};
use core::ops::Index;
use std::collections::HashMap;

#[cfg(feature = "memory_store")]
use crate::stores::MemoryStore;

pub struct MerkleProof<const DEPTH: usize> {
    pub proof: [Node; DEPTH],
    pub leaf: Node,
    pub index: u64,
    pub root: Node,
}

pub struct MerkleTree<H, S, const DEPTH: usize>
where
    H: Hasher,
    S: Store,
{
    hasher: H,
    store: S,
    zeros: Zeros<DEPTH>,
}

// Type alias for common configuration
#[cfg(feature = "memory_store")]
pub type MerkleTree32 = MerkleTree<Keccak256Hasher, MemoryStore, 32>;

// Default tree with common configuration
#[cfg(feature = "memory_store")]
impl Default for MerkleTree32 {
    fn default() -> Self {
        Self::new(Keccak256Hasher, MemoryStore::default())
    }
}

pub struct Zeros<const DEPTH: usize> {
    front: [Node; DEPTH],
    last: Node,
}

// TODO: Maybe use "typenum" crate to avoid this.
impl<const DEPTH: usize> Index<usize> for Zeros<DEPTH> {
    type Output = Node;
    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        if index < DEPTH {
            &self.front[index]
        } else if index == DEPTH {
            &self.last
        } else {
            panic!("index out of bounds");
        }
    }
}

// TODO: Implement send and sync so that the tree can be used in a concurrent context

impl<H, S, const DEPTH: usize> MerkleTree<H, S, DEPTH>
where
    H: Hasher,
    S: Store,
{
    pub fn new(hasher: H, store: S) -> Self {
        // TODO: Protect from overflow. Eg if depth is 256, then it will overflow.
        // Set a limit, maybe no more than 64?
        let mut zero = [Node::ZERO; DEPTH];
        for i in 1..DEPTH {
            zero[i] = hasher.hash(&zero[i - 1], &zero[i - 1]);
        }
        let zeros = Zeros {
            front: zero,
            last: hasher.hash(&zero[DEPTH - 1], &zero[DEPTH - 1]),
        };
        Self {
            hasher,
            store,
            zeros,
        }
    }

    /// Initialize Merkle tree with caller-provided precomputed zero hashes
    /// precomputed[i] must be the zero hash at level i (0 = leaf, DEPTH-1 = top)
    pub fn with_precomputed_zeros(hasher: H, store: S, precomputed: [Node; DEPTH]) -> Self {
        let zeros = Zeros {
            front: precomputed,
            last: hasher.hash(&precomputed[DEPTH - 1], &precomputed[DEPTH - 1]),
        };
        Self {
            hasher,
            store,
            zeros,
        }
    }

    pub fn add_leaves(&mut self, leaves: &[Node]) -> Result<(), MerkleError> {
        // Early return
        if leaves.is_empty() {
            return Ok(());
        }

        // Error if leaves do not fit in the tree
        // TODO: Avoid calculating this. Calculate it at init or do the shifting with the generic.
        if self.store.get_num_leaves() + leaves.len() as u64 > (1 << DEPTH as u64) {
            return Err(MerkleError::TreeFull {
                depth: DEPTH as u32,
                capacity: 1 << DEPTH as u64,
            });
        }

        // Stores the levels and hashes to be written in a single batch.
        // This allows to batch all writes in a single batch transaction.
        let mut batch: Vec<(u32, u64, Node)> = Vec::with_capacity(leaves.len() * (DEPTH + 1));

        // Cache for nodes generated in this batch so we can reuse them
        let mut cache: HashMap<(u32, u64), Node> = HashMap::new();

        for (offset, leaf) in leaves.iter().enumerate() {
            let mut idx = self.store.get_num_leaves() + offset as u64;
            let mut h = *leaf;

            // Store the leaf
            batch.push((0, idx, h));
            cache.insert((0, idx), h);

            // Collect siblings that are not already cached so we can fetch them in one batch.
            let mut levels_to_fetch = [0u32; DEPTH];
            let mut indices_to_fetch = [0u64; DEPTH];
            let mut fetch_len = 0usize;

            let mut tmp_idx = idx;
            for lvl in 0..DEPTH {
                let sibling_idx = tmp_idx ^ 1;
                if !cache.contains_key(&(lvl as u32, sibling_idx)) {
                    levels_to_fetch[fetch_len] = lvl as u32;
                    indices_to_fetch[fetch_len] = sibling_idx;
                    fetch_len += 1;
                }
                tmp_idx >>= 1;
            }

            // Batch-fetch the missing siblings and insert them in cache.
            if fetch_len != 0 {
                let fetched = self.store.get(
                    &levels_to_fetch[..fetch_len],
                    &indices_to_fetch[..fetch_len],
                )?;

                for (i, maybe_node) in fetched.into_iter().enumerate() {
                    if let Some(node) = maybe_node {
                        cache.insert((levels_to_fetch[i], indices_to_fetch[i]), node);
                    }
                }
            }

            for level in 0..DEPTH {
                let sibling_idx = idx ^ 1;

                let sib_hash = cache
                    .get(&(level as u32, sibling_idx))
                    .copied()
                    .unwrap_or(self.zeros[level]);

                let (left, right) = if idx & 1 == 1 {
                    (sib_hash, h)
                } else {
                    (h, sib_hash)
                };

                h = self.hasher.hash(&left, &right);
                idx >>= 1;

                batch.push(((level + 1) as u32, idx, h));
                cache.insert(((level + 1) as u32, idx), h);
            }
        }

        // Update all values in a single batch
        self.store.put(&batch)?;

        Ok(())
    }

    pub fn root(&self) -> Result<Node, MerkleError> {
        Ok(self
            .store
            .get(&[DEPTH as u32], &[0])?
            .into_iter()
            .next()
            .ok_or_else(|| MerkleError::StoreError("root fetch returned empty vector".into()))?
            .unwrap_or(self.zeros[DEPTH]))
    }

    pub fn proof(&self, leaf_idx: u64) -> Result<MerkleProof<DEPTH>, MerkleError> {
        // Implementation detail. Allow proofs even beyond the number of leaves.
        // Since it has fixed depth it is technically correct.
        // Error if leaf_idx is out of bounds.
        // if leaf_idx >= self.store.get_num_leaves() {
        //    return Err(MerkleError::LeafIndexOutOfBounds {
        //        index: leaf_idx,
        //        num_leaves: self.store.get_num_leaves(),
        //    }
        //    .into());
        //}

        if leaf_idx > 1 << DEPTH as u64 {
            return Err(MerkleError::LeafIndexOutOfBounds {
                index: leaf_idx,
                num_leaves: 1 << DEPTH as u64,
            });
        }

        // Build level/index lists for siblings plus the leaf.
        // TODO: Can't do arithmetic here with DEPTH meaning there is no
        // easy way to put this in the stack. Unfortunately the array size
        // has to be DEPTH + 1 to have a single read. One day Rust will
        // have const-generic arithmetic.
        let mut levels: Vec<u32> = Vec::with_capacity(DEPTH + 1);
        let mut indices: Vec<u64> = Vec::with_capacity(DEPTH + 1);

        let mut idx = leaf_idx;
        for level in 0..DEPTH {
            let sibling = idx ^ 1;
            levels.push(level as u32);
            indices.push(sibling);
            idx >>= 1;
        }

        // Append the leaf itself at index leaf_idx.
        levels.push(0);
        indices.push(leaf_idx);

        // Batch fetch all requested nodes.
        let fetched = self.store.get(&levels, &indices)?;

        // The first DEPTH items are the siblings.
        let mut proof = [Node::ZERO; DEPTH];
        for (d, opt) in fetched.iter().take(DEPTH).enumerate() {
            proof[d] = opt.unwrap_or(self.zeros[d]);
        }

        // The last item is the leaf itself.
        let leaf_hash = fetched.last().copied().flatten().unwrap_or(self.zeros[0]);

        Ok(MerkleProof {
            proof,
            leaf: leaf_hash,
            index: leaf_idx,
            root: self.root()?,
        })
    }

    pub fn verify_proof(&self, proof: &MerkleProof<DEPTH>) -> Result<bool, MerkleError> {
        let mut computed_hash = proof.leaf;
        for (j, sibling_hash) in proof.proof.iter().enumerate() {
            let (left, right) = if proof.index & (1 << j) == 0 {
                (computed_hash, *sibling_hash)
            } else {
                (*sibling_hash, computed_hash)
            };
            computed_hash = self.hasher.hash(&left, &right);
        }
        Ok(computed_hash == proof.root)
    }

    pub fn num_leaves(&self) -> u64 {
        self.store.get_num_leaves()
    }
}

#[cfg(test)]
mod tests {
    use crate::hasher::PoseidonHasher;

    use super::*;
    use crate::to_node;

    #[cfg(feature = "memory_store")]
    #[test]
    fn test_zero_keccak_32() {
        let hasher = Keccak256Hasher;
        let store = MemoryStore::default();
        let tree: MerkleTree32 = MerkleTree::new(hasher, store);

        // Test vector of expected zeros at each level.
        // Depth: 32
        // Hashing: Keccak256
        let expected_zeros = [
            to_node!("0x0000000000000000000000000000000000000000000000000000000000000000"),
            to_node!("0xad3228b676f7d3cd4284a5443f17f1962b36e491b30a40b2405849e597ba5fb5"),
            to_node!("0xb4c11951957c6f8f642c4af61cd6b24640fec6dc7fc607ee8206a99e92410d30"),
            to_node!("0x21ddb9a356815c3fac1026b6dec5df3124afbadb485c9ba5a3e3398a04b7ba85"),
            to_node!("0xe58769b32a1beaf1ea27375a44095a0d1fb664ce2dd358e7fcbfb78c26a19344"),
            to_node!("0x0eb01ebfc9ed27500cd4dfc979272d1f0913cc9f66540d7e8005811109e1cf2d"),
            to_node!("0x887c22bd8750d34016ac3c66b5ff102dacdd73f6b014e710b51e8022af9a1968"),
            to_node!("0xffd70157e48063fc33c97a050f7f640233bf646cc98d9524c6b92bcf3ab56f83"),
            to_node!("0x9867cc5f7f196b93bae1e27e6320742445d290f2263827498b54fec539f756af"),
            to_node!("0xcefad4e508c098b9a7e1d8feb19955fb02ba9675585078710969d3440f5054e0"),
            to_node!("0xf9dc3e7fe016e050eff260334f18a5d4fe391d82092319f5964f2e2eb7c1c3a5"),
            to_node!("0xf8b13a49e282f609c317a833fb8d976d11517c571d1221a265d25af778ecf892"),
            to_node!("0x3490c6ceeb450aecdc82e28293031d10c7d73bf85e57bf041a97360aa2c5d99c"),
            to_node!("0xc1df82d9c4b87413eae2ef048f94b4d3554cea73d92b0f7af96e0271c691e2bb"),
            to_node!("0x5c67add7c6caf302256adedf7ab114da0acfe870d449a3a489f781d659e8becc"),
            to_node!("0xda7bce9f4e8618b6bd2f4132ce798cdc7a60e7e1460a7299e3c6342a579626d2"),
            to_node!("0x2733e50f526ec2fa19a22b31e8ed50f23cd1fdf94c9154ed3a7609a2f1ff981f"),
            to_node!("0xe1d3b5c807b281e4683cc6d6315cf95b9ade8641defcb32372f1c126e398ef7a"),
            to_node!("0x5a2dce0a8a7f68bb74560f8f71837c2c2ebbcbf7fffb42ae1896f13f7c7479a0"),
            to_node!("0xb46a28b6f55540f89444f63de0378e3d121be09e06cc9ded1c20e65876d36aa0"),
            to_node!("0xc65e9645644786b620e2dd2ad648ddfcbf4a7e5b1a3a4ecfe7f64667a3f0b7e2"),
            to_node!("0xf4418588ed35a2458cffeb39b93d26f18d2ab13bdce6aee58e7b99359ec2dfd9"),
            to_node!("0x5a9c16dc00d6ef18b7933a6f8dc65ccb55667138776f7dea101070dc8796e377"),
            to_node!("0x4df84f40ae0c8229d0d6069e5c8f39a7c299677a09d367fc7b05e3bc380ee652"),
            to_node!("0xcdc72595f74c7b1043d0e1ffbab734648c838dfb0527d971b602bc216c9619ef"),
            to_node!("0x0abf5ac974a1ed57f4050aa510dd9c74f508277b39d7973bb2dfccc5eeb0618d"),
            to_node!("0xb8cd74046ff337f0a7bf2c8e03e10f642c1886798d71806ab1e888d9e5ee87d0"),
            to_node!("0x838c5655cb21c6cb83313b5a631175dff4963772cce9108188b34ac87c81c41e"),
            to_node!("0x662ee4dd2dd7b2bc707961b1e646c4047669dcb6584f0d8d770daf5d7e7deb2e"),
            to_node!("0x388ab20e2573d171a88108e79d820e98f26c0b84aa8b2f4aa4968dbb818ea322"),
            to_node!("0x93237c50ba75ee485f4c22adf2f741400bdf8d6a9cc7df7ecae576221665d735"),
            to_node!("0x8448818bb4ae4562849e949e17ac16e0be16688e156b5cf15e098c627c0056a9"),
            to_node!("0x27ae5ba08d7291c96c8cbddcc148bf48a6d68c7974b94356f53754ef6171d757"),
        ];

        for (i, zero) in tree.zeros.front.iter().enumerate() {
            assert_eq!(zero, &expected_zeros[i]);
        }
        assert_eq!(tree.zeros.last, expected_zeros[32]);
    }

    #[cfg(feature = "memory_store")]
    #[test]
    fn test_zero_poseidon_32() {
        let hasher = PoseidonHasher;
        let store = MemoryStore::default();
        let tree = MerkleTree::<PoseidonHasher, MemoryStore, 32>::new(hasher, store);

        // Test vector of expected zeros at each level.
        // Depth: 32
        // Hashing: Poseidon
        // See. Note in int not hex
        // https://github.com/zk-kit/zk-kit.solidity/blob/lean-imt.sol-v2.0.1/packages/lazy-imt/contracts/InternalLazyIMT.sol#L16-L48
        let expected_zeros = [
            to_node!("0x0000000000000000000000000000000000000000000000000000000000000000"),
            to_node!("0x2098f5fb9e239eab3ceac3f27b81e481dc3124d55ffed523a839ee8446b64864"),
            to_node!("0x1069673dcdb12263df301a6ff584a7ec261a44cb9dc68df067a4774460b1f1e1"),
            to_node!("0x18f43331537ee2af2e3d758d50f72106467c6eea50371dd528d57eb2b856d238"),
            to_node!("0x07f9d837cb17b0d36320ffe93ba52345f1b728571a568265caac97559dbc952a"),
            to_node!("0x2b94cf5e8746b3f5c9631f4c5df32907a699c58c94b2ad4d7b5cec1639183f55"),
            to_node!("0x2dee93c5a666459646ea7d22cca9e1bcfed71e6951b953611d11dda32ea09d78"),
            to_node!("0x078295e5a22b84e982cf601eb639597b8b0515a88cb5ac7fa8a4aabe3c87349d"),
            to_node!("0x2fa5e5f18f6027a6501bec864564472a616b2e274a41211a444cbe3a99f3cc61"),
            to_node!("0x0e884376d0d8fd21ecb780389e941f66e45e7acce3e228ab3e2156a614fcd747"),
            to_node!("0x1b7201da72494f1e28717ad1a52eb469f95892f957713533de6175e5da190af2"),
            to_node!("0x1f8d8822725e36385200c0b201249819a6e6e1e4650808b5bebc6bface7d7636"),
            to_node!("0x2c5d82f66c914bafb9701589ba8cfcfb6162b0a12acf88a8d0879a0471b5f85a"),
            to_node!("0x14c54148a0940bb820957f5adf3fa1134ef5c4aaa113f4646458f270e0bfbfd0"),
            to_node!("0x190d33b12f986f961e10c0ee44d8b9af11be25588cad89d416118e4bf4ebe80c"),
            to_node!("0x22f98aa9ce704152ac17354914ad73ed1167ae6596af510aa5b3649325e06c92"),
            to_node!("0x2a7c7c9b6ce5880b9f6f228d72bf6a575a526f29c66ecceef8b753d38bba7323"),
            to_node!("0x2e8186e558698ec1c67af9c14d463ffc470043c9c2988b954d75dd643f36b992"),
            to_node!("0x0f57c5571e9a4eab49e2c8cf050dae948aef6ead647392273546249d1c1ff10f"),
            to_node!("0x1830ee67b5fb554ad5f63d4388800e1cfe78e310697d46e43c9ce36134f72cca"),
            to_node!("0x2134e76ac5d21aab186c2be1dd8f84ee880a1e46eaf712f9d371b6df22191f3e"),
            to_node!("0x19df90ec844ebc4ffeebd866f33859b0c051d8c958ee3aa88f8f8df3db91a5b1"),
            to_node!("0x18cca2a66b5c0787981e69aefd84852d74af0e93ef4912b4648c05f722efe52b"),
            to_node!("0x2388909415230d1b4d1304d2d54f473a628338f2efad83fadf05644549d2538d"),
            to_node!("0x27171fb4a97b6cc0e9e8f543b5294de866a2af2c9c8d0b1d96e673e4529ed540"),
            to_node!("0x2ff6650540f629fd5711a0bc74fc0d28dcb230b9392583e5f8d59696dde6ae21"),
            to_node!("0x120c58f143d491e95902f7f5277778a2e0ad5168f6add75669932630ce611518"),
            to_node!("0x1f21feb70d3f21b07bf853d5e5db03071ec495a0a565a21da2d665d279483795"),
            to_node!("0x24be905fa71335e14c638cc0f66a8623a826e768068a9e968bb1a1dde18a72d2"),
            to_node!("0x0f8666b62ed17491c50ceadead57d4cd597ef3821d65c328744c74e553dac26d"),
            to_node!("0x0918d46bf52d98b034413f4a1a1c41594e7a7a3f6ae08cb43d1a2a230e1959ef"),
            to_node!("0x1bbeb01b4c479ecde76917645e404dfa2e26f90d0afc5a65128513ad375c5ff2"),
            to_node!("0x2f68a1c58e257e42a17a6c61dff5551ed560b9922ab119d5ac8e184c9734ead9"),
        ];

        for (i, zero) in tree.zeros.front.iter().enumerate() {
            assert_eq!(zero, &expected_zeros[i]);
        }
        assert_eq!(tree.zeros.last, expected_zeros[32]);
    }

    #[cfg(feature = "memory_store")]
    #[test]
    fn test_tree_full_error() {
        let hasher = Keccak256Hasher;
        let store = MemoryStore::default();
        let mut tree = MerkleTree::<Keccak256Hasher, MemoryStore, 3>::new(hasher, store);

        tree.add_leaves(&(0..8).map(|_| Node::ZERO).collect::<Vec<Node>>())
            .unwrap();

        // It errors since the tree is full
        assert!(tree.add_leaves(&[Node::ZERO]).is_err());
    }
}
