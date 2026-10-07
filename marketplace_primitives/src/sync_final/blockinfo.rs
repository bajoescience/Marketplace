use borsh::{BorshDeserialize, BorshSerialize};

use crate::{Block, Proposer, Validator};

/// Full information concerning a `Block` which includes 
/// the following:
/// 
/// - Block
/// 
/// - Proposer
/// 
/// - List of Validators
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BlockInfo {
    block: Block,
    proposer: Proposer,
    validators: Vec<Validator>
}

impl BlockInfo {
    pub fn new(block: Block, proposer: Proposer, validators: Vec<Validator>) -> Self {
        Self {
            block,
            proposer,
            validators
        }
    }

    /// Create block from block bytes
    pub fn from(bytes: &[u8]) -> Result<Self, std::io::Error> {
        borsh::from_slice(bytes)
    }

    // Serialize
    pub fn serialize(&self) -> Vec<u8> {
        borsh::to_vec(&self)
            .expect("Illegal: Error Serializing")
    }
}

// Getter methods
impl BlockInfo {
    // Get block
    pub fn block(&self) -> &Block {
        &self.block
    }
}