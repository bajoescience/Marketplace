/// ## Synchronous Finalization
pub mod block;
pub mod blockinfo;
pub mod voter;

pub use self::block::{BlockHeader, Block, BlockBody};
pub use voter::{Proposer, Validator, Proposal, Validated};

#[cfg(test)]
mod tests {
    use super::{Block, BlockBody, BlockHeader};

    // The synchronous-finalization module should expose its core block types.
    #[test]
    fn block_types_are_available_through_the_module() {
        let genesis = Block::genesis();
        let body = BlockBody::genesis();

        assert_eq!(genesis.header().index(), 0);
        assert_eq!(body.len(), 1);
        assert_eq!(BlockHeader::genesis().index(), 0);
    }
}