/// ## Synchronous Finalization
pub mod block;
pub mod proposal;
pub mod validator;
pub mod blockinfo;

pub use self::block::{BlockHeader, Block, BlockBody, SubBlock};
pub use self::proposal::{Proposer, Proposal};
pub use self::validator::{Validator, Validated};