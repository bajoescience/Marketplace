use marketplace_helpers::objects::{AgentResult, WU};

use crate::wallet::{Lock, Key};

use super::{BorshDeserialize, BorshSerialize, ID};

/// # Transaction Output `TxOpt` 
/// 
/// Structure representing an Output in a Transaction.
/// 
/// ## Examples
/// 
/// A user sending money to themselves by creating an output to add to a transaction.
/// 
/// ```
/// use marketplace_wallet::{Owner};
/// use marketplace_helpers::objects::WU;
/// use marketplace_primitives::tx::TxOpt;
/// 
/// let user = Owner::new_sig();
/// let lock = user.as_lock(); 
/// 
/// let output = TxIO::new(lock, WU::GDC());
/// 
/// // Output is equal to one gdc
/// assert_eq!(output.amount(), WU::GDC());
/// ```
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct TxOpt {
    // Owner
    lock: Lock,

    // Amount 
    amount: WU
}

// Setter methods
impl TxOpt {
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
impl TxOpt {
    pub fn lock(&self) -> &Lock {
        &self.lock
    }

    // Input amount
    pub fn amount(&self) -> WU {
        self.amount
    }
}

/// # Transaction Input `TxOptRef` 
/// 
/// Structure representing an Input in a Transaction which references a Tx output.
/// 
/// ## Examples
/// 
/// A user sending money to themselves by creating an Input to add to a transaction.
/// 
/// ```
/// use marketplace_wallet::{Owner};
/// use marketplace_helpers::objects::WU;
/// use marketplace_primitives::tx::TxOptRef;
/// 
/// let user = Owner::new_sig();
/// 
/// // Dummy transaction id
/// let tx = [255u8; 32]; 
/// 
/// // Input referencing Transaction tx at output 0
/// let input = TxOptRef::new(tx, 0);
/// 
/// // Input is equal to one gdc
/// assert_eq!(input.amount(), WU::GDC());
/// ```
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct TxOptRef {
    // Transaction id
    // where referenced output is
    tx: ID,

    // Output index in Transaction output list
    index: usize,
}

impl TxOptRef {
    pub fn new(tx: ID, index: usize) -> Self {
        Self {
            tx,
            index
        }
    }
}

// Getter methods
impl TxOptRef {
    pub fn tx(&self) -> &[u8] {
        &self.tx
    }

    pub fn index(&self) -> usize {
        self.index
    }
}