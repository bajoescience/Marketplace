pub mod state;
mod job;
mod blockchain;
mod mempool;

pub use state::{Balances, State, DefaultBalances};
pub use job::Jobs;
pub use blockchain::Blockchain;
pub use mempool::Mempool;

#[cfg(test)]
mod tests {
    use marketplace_wallet::Owner;

    use crate::{DefaultBalances, Jobs, Mempool, State};

    // The crate should expose its primary ledger state and coordination types.
    #[test]
    fn public_ledger_types_can_be_constructed() {
        let state = State::<DefaultBalances>::new();
        let jobs = Jobs::new();
        let mut mempool = Mempool::new();
        let owner = Owner::new_sig();

        assert_eq!(
            state.get_gdc_balance(owner.as_lock()),
            marketplace_helpers::objects::WU::default()
        );
        assert!(mempool.get_proposal_to_finalize().is_none());
        drop(jobs);
    }
}
