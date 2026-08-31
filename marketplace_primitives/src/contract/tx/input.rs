use marketplace_helpers::objects::{AgentResult, WU};

use crate::wallet::{Lock, Key};

use super::{BorshDeserialize, BorshSerialize, ID};

// Object refrencing another contracts output
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct TxIO {
    // Owner of input pubkey hash
    lock: Lock,

    // Amount 
    amount: WU
}

// Setter methods
impl TxIO {
    /// owner: This is the Lock of the entity that owns
    /// some output coins in the tx which is to be unlocked
    pub fn new(lock: Lock, amount: WU) -> Self {
        Self {
            lock,
            amount
        }
    }

    pub fn unlock(&self, key: &Key, tx_id: &ID) -> AgentResult<()> {
        self.lock.unlock(key, tx_id)
    }
}

// Getter methods
impl TxIO {
    pub fn lock(&self) -> &Lock {
        &self.lock
    }

    // Input amount
    pub fn amount(&self) -> WU {
        self.amount
    }
}