use std::vec;

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{ID, IdHash, WU}};
use rs_merkle::{MerkleTree, algorithms::Sha256};

use crate::{Contract, WRVote};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy)]
pub struct BlockHeader {
    // Contract merkle root
    ctr_merkle_root: Option<ID>,

    // Previous block ID
    prev: ID,

    // Total amount of new gdc in block
    new_gdc: WU,

    // New average historical gdc created per block
    average: WU,
}

// Associated functions
impl BlockHeader {
    pub fn genesis() -> Self {
        let genesis = Block::genesis();

        Self {
            ctr_merkle_root: genesis.root(),
            prev: [0u8; 32],
            new_gdc: WU::try_from(100).unwrap() * WU::GDC(),
            average: WU::single(),
        }
    }

    // New Block Header instance
    // Usually created with Blockchain
    pub fn new(
        blk: &Block, 
        prev_header: &BlockHeader,
    ) -> Self {
        let new_avg = functions::avg_wu(
            blk.total(),
            prev_header.average() 
        );

        Self {
            ctr_merkle_root: blk.root(),
            prev: prev_header.id(),
            new_gdc: blk.total(),
            average: new_avg,
        }
    }
}

// Getter methods
impl BlockHeader {
    // Get amount of new gdc in block
    pub fn new_gdc(&self) -> WU {
        self.new_gdc
    }

    // Get average
    pub fn average(&self) -> WU {
        self.average
    }
}

impl PartialEq for BlockHeader {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl WRVote for BlockHeader {
    fn as_vote(&self) -> ID {
        self.id()
    }
}

// Block stores list of contracts
// Empty blocks can exist
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct Block {
    body: Vec<Contract>,
}

impl Block {
    /// Create a new Block from a Vec of contracts
    /// 
    /// An empty Vector can also be supplied, but it will 
    /// have a VRF_threshold that
    /// will ultimately be half the original
    pub fn new(body: Vec<Contract>) -> Self {
        Self {
            body,
        }
    }

    // Genesis Block
    pub fn genesis() -> Self {
        Self {
            body: vec![Contract::genesis()]
        }  
    }

    // Serialize
    pub fn serialize(&self) -> Vec<u8> {
        borsh::to_vec(&self)
            .expect("Illergal: Error Serializing")
    }
}

// Getter methods
impl Block {
    // Total amount of new gdc coins in block
    pub fn total(&self) -> WU {
        self.body
        .iter()
        .map(|ctr| ctr.new_coins())
        .sum()
    }

    // The id of the block is the merkle root hash
    // of all the contracts
    pub fn root(&self) -> Option<ID> {
        // Hash every contract as the leaf of the merkel tree
        let leaves: Vec<ID> = self.body
            .iter()
            .map(|ctr| ctr.id())
            .collect();
        
        // Merkle tree
        let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaves);
        merkle_tree.root()
    }

    // Block body
    pub fn body(&self) -> &[Contract] {
        &self.body
    }

    // Number of Contracts in block
    pub fn len(&self) -> usize {
        self.body.len()
    }
}

#[cfg(test)]
mod tests {
    use std::assert_eq;

use super::*;

    // Valid block
    #[test]
    fn valid_block() {
        let block = Block::genesis();

        assert_eq!(block.total(), WU::default());
        assert_eq!(block.len(), 1);
    }
}