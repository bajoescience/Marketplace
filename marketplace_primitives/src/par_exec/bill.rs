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

