//! # Bill
//! 
//! A list of related transactions to be finalized by the network

use borsh::{BorshDeserialize, BorshSerialize};

use crate::par_exec::Tx;

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Bill(Vec<Tx>);

impl Bill {
    pub fn new(bill: Vec<Tx>) -> Self {
        Self(bill)
    }

    // Initialize genesis Bill for currencies
    pub fn genesis() -> Self {
        Self (vec![Tx::genesis(), Tx::gtt()])
    }
}

// Getter methods
impl Bill {
    pub fn get_txs(&self) -> &Vec<Tx> {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use crate::par_exec::{Bill, Tx, TxIdentifier};

    // A bill should retain the transactions supplied to its constructor.
    #[test]
    fn new_bill_exposes_its_transactions() {
        let bill = Bill::new(vec![Tx::genesis()]);

        assert_eq!(bill.get_txs().len(), 1);
        assert!(bill.get_txs()[0].is_coin());
    }

    // The genesis bill should contain the initial coin and token transactions.
    #[test]
    fn genesis_bill_contains_initial_currency_transactions() {
        let bill = Bill::genesis();

        assert_eq!(bill.get_txs().len(), 2);
        assert!(matches!(bill.get_txs()[0].tid(), TxIdentifier::COIN));
        assert!(matches!(bill.get_txs()[1].tid(), TxIdentifier::TOKEN(Some(_))));
    }
}
