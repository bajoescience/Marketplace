use std::collections::{HashMap, hash_map::Entry};

use borsh::{BorshDeserialize, BorshSerialize};

use marketplace_helpers::{objects::{AgentResult, ID, IdHash}, functions};
use marketplace_wallet::{Key, Lock};

use crate::{Tx, Verify};

// Contract type for casual transaction
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TxContract {
    bill: Vec<Tx>,

    // Authenticate TxContract
    // Stores <Lock.id(), Key>
    #[borsh(skip)]
    auth: HashMap<ID, Key>
}

impl TxContract {
    pub fn new(bill: Vec<Tx>) -> Self {
        Self {
            bill,
            auth: HashMap::new(),
        }
    }

    // Initialize genesis JobContract for goldcoin
    pub fn genesis() -> Self {
        Self {
            bill: vec![Tx::genesis(), Tx::gtt()],
            auth: HashMap::new(),
        }
    }

    // force parameter will overide any previous associated keys
    pub fn add_auth(&mut self, lock: Lock, key: Key, force: bool) -> AgentResult<()> {
        match self.auth.entry(lock.id()) {
            Entry::Vacant(entry) => {
                entry.insert(key);
            },
            Entry::Occupied(mut entry) if force => {
                entry.insert(key);
            },
            Entry::Occupied(_) => {
                return Err(format!(
                    "Key already exists for Lock ID {}",
                    functions::from_bytes(&lock.id())
                ));
            },  
        }

        Ok(())
    }
}

// Setter methods
impl TxContract {
    pub fn get_tx(&self) -> impl Iterator<Item = &Tx> {
        self.bill
            .iter()
    } 
}

impl Verify for TxContract {
    fn verify(&self) -> AgentResult<()> {
        for tx in &self.bill {
            // Verify Tx
            tx.verify()?;

            // Unlock tx
            tx.unlock(&self.auth, &self.id())?;
        }

        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use std::{vec};

use marketplace_helpers::{ objects::{IdHash, WU}};
use marketplace_wallet::{Owner};

use crate::{ TxIO, TxIdentifier};

use super::*;

    // New txcontract
    #[test]
    fn verify_new_tx_contract() -> AgentResult<()> {
        let owner = Owner::new_sig();

        let ipts = vec![
            TxIO::new(owner.as_lock(), WU::try_from(20000).unwrap())
        ];

        let opts = Some(vec![
            TxIO::new(owner.as_lock(), WU::try_from(20000).unwrap())
        ]);
        
        let tx = Tx::new(0, TxIdentifier::COIN, Some(ipts), opts);

        let mut txctr = TxContract::new(vec![tx]);
        let key = owner.sign(&txctr.id());

        txctr.add_auth(owner.as_lock(), key, false)?;

        txctr.verify()
    }
}