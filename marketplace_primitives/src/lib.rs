// Parallel Execution
pub mod par_exec;


// Synchronous finalization
pub mod sync_final;

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