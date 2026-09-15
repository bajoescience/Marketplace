mod block;

// Parallel Execution
mod par_exec;

pub use self::par_exec::{
    Tx, Contract, TxIO, TxIdentifier, Whiteroom, 
    WRVote, WorkPtr, ResultPtr, JobContract, TxContract,
    ExecuteTime, ResultInfo
};

pub use self::block::{Block, BlockHeader};

use marketplace_helpers as helpers;
use marketplace_wallet as wallet;

// Verify the integrity of all cryptographic primitives in the structure
pub trait Verify {
    // Verify whether the structure is valid and correct
    fn verify(&self) -> helpers::objects::AgentResult<()>;
}

// // Trait that defines a whiteroom member
// pub trait Member: IdHash + Clone {}

// impl<T> Member for T where T: IdHash + Clone {}