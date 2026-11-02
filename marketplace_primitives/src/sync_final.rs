/// ## Synchronous Finalization
pub mod block;
pub mod blockinfo;
pub mod voter;

pub use self::block::{BlockHeader, Block, BlockBody};
pub use voter::{Proposer, Validator, Proposal, Validated};