use std::collections::{HashMap, hash_map::Entry};

use borsh::{BorshDeserialize, BorshSerialize};

use marketplace_helpers::{objects::{AgentResult, ID, IdHash}};
use marketplace_wallet::{Key, Lock};

use crate::{Verify, par_exec::{Bill, Tx}};

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TxContract {
    bill: Bill,

    // Authenticate TxContract
    // Stores <Lock.id(), Key>
    // After signing bill
    auth: HashMap<ID, Key>
}

impl TxContract {
    pub fn new(bill: Bill) -> Self {
        Self {
            bill,
            auth: HashMap::new(),
        }
    }

    // Create new instance with auth
    pub fn new_with_auth(bill: Bill, auth: HashMap<ID, Key>) -> Self {
        Self {
            bill,
            auth,
        }
    }

    // Initialize genesis JobContract for goldcoin
    pub fn genesis() -> Self {
        Self {
            bill: Bill::genesis(),
            auth: HashMap::new(),
        }
    }

    // force parameter will overide any previous associated keys
    pub fn add_auth(&mut self, lock: Lock, key: Key) {
        match self.auth.entry(lock.id()) {
            Entry::Vacant(entry) => {
                entry.insert(key);
            },
            Entry::Occupied(_) => {
                
            },  
        }
    }
}

// Getter methods
impl TxContract {
    pub fn id(&self) -> ID {
        self.bill.id()
    }

    pub fn bill(&self) -> &Bill {
        &self.bill
    }

    pub fn auth(&self) -> &HashMap<ID, Key> {
        &self.auth
    }
}

// Setter methods
impl TxContract {
    pub fn get_txs(&self) -> &Vec<Tx> {
        self.bill.get_txs()
    } 
}

impl Verify for TxContract {
    fn verify(&self) -> AgentResult<()> {
        for tx in self.bill.get_txs().iter() {
            // Verify the structure of the Tx
            tx.verify()?;
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use std::{vec};

use marketplace_helpers::{ objects::{IdHash, WU}};
use marketplace_wallet::{Owner};

use crate::par_exec::{ TxIdentifier, tx::{TxOpt, TxOptRef}};

use super::*;

    // New txcontract
    #[test]
    fn verify_new_tx_contract() -> AgentResult<()> {
        let owner = Owner::new_sig();

        let ipts = vec![
            TxOptRef::new([255; 32], 0)
        ];

        let opts = Some(vec![
            TxOpt::new(owner.as_lock(), WU::try_from(20000).unwrap())
        ]);
        
        let tx = Tx::new(0, TxIdentifier::COIN, Some(ipts), opts);

        let bill = Bill::new(vec![tx]);
        let key = owner.sign(&bill.id());

        let mut txctr = TxContract::new(bill);

        txctr.add_auth(owner.as_lock(), key);

        txctr.verify()
    }
}