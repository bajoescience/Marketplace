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
}