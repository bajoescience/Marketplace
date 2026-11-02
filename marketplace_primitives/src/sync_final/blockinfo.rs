use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::functions;

use crate::{
    par_exec::Whiteroom,
    sync_final::{Block, Validated},
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
    block: Block,
    validators: Whiteroom<Validated, WHITEROOM_MAX>
}

impl BlockInfo {
    pub fn new(block: Block, validators: Whiteroom<Validated, WHITEROOM_MAX>) -> Self {
        Self {
            block,
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

    // Get Validators
    pub fn validated(&self) -> &Whiteroom<Validated, WHITEROOM_MAX> {
        &self.validators
    }
}