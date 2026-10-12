use std::vec;
use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::objects::{CONTRACT_WHITEROOM_SIZE, ID, IdHash, WU};
use rs_merkle::{MerkleTree, algorithms::Sha256};

use crate::{Contract, sync_final::block::subblock::SubBlock};

// Block stores list of contracts
// Empty blocks can exist
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BlockBody {
    list: Vec<SubBlock>,
}

impl BlockBody {
    pub fn new() -> Self {
        Self {
            list: Vec::new(),
        }
    }

    // Genesis Block
    pub fn genesis() -> Self {
        Self {
            list: vec![SubBlock::genesis()]
        }  
    }
}

// Getter methods
impl BlockBody {
    pub fn contents(&self) -> impl Iterator<Item = &SubBlock> {
        self.list.iter()
    }

    // Average whiteroom size of every subblock
    // in the block
    pub fn wr_avg(&self) -> usize {
        let wr_sum: usize = self.contents()
            .map(|subblock| subblock.wr_avg())
            .sum();

        (wr_sum / self.len()) as usize
    }

    // Total amount of new gdc coins in block
    pub fn total(&self) -> WU {
        self.contents()
        .map(|subblock| subblock.total())
        .sum()
    }

    // The id of the block is the merkle root hash
    // of all the contracts
    pub fn root(&self) -> Option<ID> {
        // Hash every contract as the leaf of the merkel tree
        let leaves: Vec<ID> = self.contents()
            .map(|sub_block| sub_block.id())
            .collect();
        
        // Merkle tree
        let merkle_tree = MerkleTree::<Sha256>::from_leaves(&leaves);
        merkle_tree.root()
    }

    // Block body
    pub fn ctrs(&self) -> Vec<&Contract> {
        self.contents()
            .map(|subblock| subblock.contents())
            .flatten()
            .collect()
    }

    /// Number of Contracts in block
    pub fn len(&self) -> usize {
        self.contents()
            .map(|subblock| subblock.len())
            .sum()
    }
}

// Setter methods
impl BlockBody {
    /// Add a sub block to the blockbody
    pub fn add(&mut self, subblock: SubBlock) {
        self.list.push(subblock);
    }
}