use marketplace_helpers::objects::{AgentResult, WU};

use crate::wallet::{Lock, Key};

use super::{BorshDeserialize, BorshSerialize, ID};

/// # Transaction Input/Output `TXIO` 
/// 
/// Structure representing an Input or Output in a Transaction.
/// 
/// ## Examples
/// 
/// A user sending money to themselves by creating an input,
/// and an output to add to a transaction.
/// 
/// ```
/// use marketplace_wallet::{Owner};
/// use marketplace_helpers::objects::WU;
/// use marketplace_primitives::TxIO;
/// 
/// let user = Owner::new_sig();
/// let lock = user.as_lock();
/// 
/// let input = TxIO::new(lock, WU::GDC());
/// let output = TxIO::new(lock, WU::GDC());
/// 
/// // Input is equal to output
/// assert_eq!(input.amount(), output.amount());
/// ```
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub struct TxIO {
    // Owner
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