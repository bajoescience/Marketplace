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
    auth: Whiteroom<Validated, WHITEROOM_MAX>
}

impl BlockInfo {
    pub fn new(block: Block, auth: Whiteroom<Validated, WHITEROOM_MAX>) -> Self {
        Self {
            block,
            auth
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

    /// Break block info into block and auth parts
    /// taking owneship of the blockinfo and returning owned values
    pub fn destroy(self) -> (Block, Whiteroom<Validated, WHITEROOM_MAX>) {
        (self.block, self.auth)
    }
}

// Getter methods
impl BlockInfo {
    // Get block
    pub fn block(&self) -> &Block {
        &self.block
    }

    // Get Validators
    pub fn auth(&self) -> &Whiteroom<Validated, WHITEROOM_MAX> {
        &self.auth
    }
}

#[cfg(test)]
mod tests {
    use marketplace_helpers::functions;
    use marketplace_wallet::voting::EXPECTED_VOTERS;

    use crate::{
        par_exec::Whiteroom,
        sync_final::{Block, Validated, blockinfo::BlockInfo},
    };

    const MAX: usize = functions::max_wr_size_from_expected(EXPECTED_VOTERS);

    // Block information should preserve its block and authenticator through serialization.
    #[test]
    fn block_info_serialization_round_trip() -> Result<(), std::io::Error> {
        let block = Block::genesis();
        let info = BlockInfo::new(block.clone(), Whiteroom::<Validated, MAX>::new());

        assert_eq!(info.block().id(), block.id());
        assert_eq!(info.auth().len(), 0);

        let restored = BlockInfo::from(&info.serialize())?;
        assert_eq!(restored.block().id(), block.id());
        assert_eq!(restored.auth().len(), 0);
        Ok(())
    }

    // Destroying block information should return its owned block and authenticator.
    #[test]
    fn destroy_returns_block_and_authenticator() {
        let block = Block::genesis();
        let info = BlockInfo::new(block.clone(), Whiteroom::<Validated, MAX>::new());
        let (restored_block, auth) = info.destroy();

        assert_eq!(restored_block.id(), block.id());
        assert_eq!(auth.len(), 0);
    }
}