/// ## Synchronous Finalization
pub mod block;
pub mod proposal;

use borsh::{BorshDeserialize, BorshSerialize};

pub use self::block::{BlockHeader, Block};
pub use self::proposal::Proposal;