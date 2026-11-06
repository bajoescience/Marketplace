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

#[cfg(test)]
mod tests {
    use marketplace_helpers::objects::WU;
    use marketplace_wallet::Owner;

    use crate::{
        Verify,
        par_exec::{Tx, TxIdentifier, TxOpt, TxOptRef},
    };

    // The crate-level Verify trait should validate a well-formed coin transaction.
    #[test]
    fn verify_trait_accepts_a_structurally_valid_transaction() {
        let owner = Owner::new_sig();
        let tx = Tx::new(
            1,
            TxIdentifier::COIN,
            Some(vec![TxOptRef::new([1; 32], 0)]),
            Some(vec![TxOpt::new(owner.as_lock(), WU::GDC())]),
        );

        assert!(tx.verify().is_ok());
    }
}