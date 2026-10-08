use std::collections::{HashMap, hash_map::Entry};

use borsh::{BorshDeserialize, BorshSerialize};

use marketplace_helpers::{objects::{AgentResult, ID, IdHash}, functions};
use marketplace_wallet::{Key, Lock};

use crate::{Tx, Verify};

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
    pub fn get_tx(&self) -> &Vec<Tx> {
        &self.bill
    } 
}

impl Verify for TxContract {
    fn verify(&self) -> AgentResult<()> {
        for tx in &self.bill {
            // Verify Tx
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

use crate::{ TxIdentifier, tx::{TxOpt, TxOptRef}};

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

        let mut txctr = TxContract::new(vec![tx]);
        let key = owner.sign(&txctr.id());

        txctr.add_auth(owner.as_lock(), key, false)?;

        txctr.verify()
    }

    // A transaction contract should preserve the transactions supplied to it.
    #[test]
    fn transaction_contract_gets_transactions() {
        let owner = Owner::new_sig();
        let amount = WU::try_from(20000).unwrap();
        let tx = Tx::new(
            0,
            TxIdentifier::COIN,
            Some(vec![TxOptRef::new([255; 32], 0)]),
            Some(vec![TxOpt::new(owner.as_lock(), amount)]),
        );
        let contract = TxContract::new(vec![tx.clone()]);

        assert_eq!(contract.get_tx().len(), 1);
        assert_eq!(contract.get_tx()[0], tx);
    }

    // Duplicate transaction-contract authentication should require force to replace it.
    #[test]
    fn transaction_contract_auth_can_be_replaced() -> AgentResult<()> {
        let owner = Owner::new_sig();
        let mut contract = TxContract::new(Vec::new());
        let first_key = owner.sign(&contract.id());
        let second_key = owner.sign(&contract.id());

        contract.add_auth(owner.as_lock(), first_key, false)?;
        let err = contract
            .add_auth(owner.as_lock(), second_key, false)
            .unwrap_err();
        assert!(err.starts_with("Key already exists for Lock ID "));
        contract.add_auth(
            owner.as_lock(),
            owner.sign(&contract.id()),
            true,
        )
    }

    // Genesis transaction contracts should contain the native and token genesis transactions.
    #[test]
    fn genesis_transaction_contract_contains_two_transactions() {
        let contract = TxContract::genesis();

        assert_eq!(contract.get_tx().len(), 2);
        assert!(contract.get_tx()[0].is_coin());
        assert!(!contract.get_tx()[1].is_coin());
    }
}