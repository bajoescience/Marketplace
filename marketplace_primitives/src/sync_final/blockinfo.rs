use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::functions;

use crate::{
    par_exec::Whiteroom, sync_final::{Block, Proposal, Validated},
};

use marketplace_wallet::voting::EXPECTED_VOTERS;

// MAX whiteroom members for validators
const WHITEROOM_MAX: usize = functions::max_wr_size_from_expected(EXPECTED_VOTERS);

/// Full information concerning a `Block` which includes 
/// the following:
/// 
/// - Block
/// 
/// - List of Validators
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize)]
pub struct BlockInfo {
    proposal: Proposal,
    validators: Whiteroom<Validated, WHITEROOM_MAX>
}

impl BlockInfo {
    pub fn new(proposal: Proposal, validators: Whiteroom<Validated, WHITEROOM_MAX>) -> Self {
        Self {
            proposal,
            validators
        }
    }
}

// Getter methods
impl BlockInfo {
    // Get proposal
    pub fn proposal(&self) -> &Proposal {
        &self.proposal
    }

    // Get block
    pub fn block(&self) -> &Block {
        &self.proposal().block()
    }

    // Get Validators
    pub fn validators(&self) -> &Whiteroom<Validated, WHITEROOM_MAX> {
        &self.validators
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}