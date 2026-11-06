//! ## Parallel Execution

pub mod work;
pub mod whiteroom;
pub mod result;
pub mod contract;
pub mod tx;
pub mod bill;


pub use self::work::{WorkPtr, ExecuteTime};
pub use self::result::{ResultPtr, ResultInfo};
pub use self::whiteroom::{Whiteroom, WRVote};
pub use self::tx::{Tx, TxIdentifier, TxOpt, TxOptRef};
pub use self::bill::Bill;
pub use self::contract::{Contract, JobContract, TxContract};

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
