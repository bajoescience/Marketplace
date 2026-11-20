//! ## Parallel Execution

pub mod bill;
pub mod contract;
pub mod result;
pub mod tx;
pub mod whiteroom;
pub mod work;

pub use self::bill::Bill;
pub use self::contract::{Contract, JobContract, TxContract};
pub use self::result::{ResultInfo, ResultPtr};
pub use self::tx::{Tx, TxIdentifier, TxOpt, TxOptRef};
pub use self::whiteroom::{WRVote, Whiteroom};
pub use self::work::{ExecuteTime, WorkPtr};

#[cfg(test)]
mod tests {
    use super::{Bill, Contract};

    // Parallel-execution re-exports should provide the genesis bill and contract.
    #[test]
    fn genesis_values_are_available_through_the_module() {
        let bill = Bill::genesis();
        let contract = Contract::genesis();

        assert_eq!(bill.get_txs().len(), 2);
        assert!(matches!(contract, Contract::TX(_)));
    }
}
