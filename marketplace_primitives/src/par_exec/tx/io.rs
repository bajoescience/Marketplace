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
/// use marketplace_primitives::par_exec::TxOpt;
/// 
/// let user = Owner::new_sig();
/// let lock = user.as_lock(); 
/// 
/// let output = TxOpt::new(lock, WU::GDC());
/// 
/// // Output is equal to one gdc
/// assert_eq!(output.amount(), WU::GDC());
/// ```
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
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
    pub fn lock(&self) -> Lock {
        self.lock
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
/// use marketplace_primitives::par_exec::TxOptRef;
/// 
/// let user = Owner::new_sig();
/// 
/// // Dummy transaction id
/// let tx = [255u8; 32]; 
/// 
/// // Input referencing Transaction tx at output 0
/// let input = TxOptRef::new(tx, 0);
/// ```
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq)]
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

#[cfg(test)]
mod tests {
    use marketplace_helpers::objects::WU;
    use marketplace_wallet::Owner;

    use super::{TxOpt, TxOptRef};

    // Transaction outputs should retain their owner lock and amount.
    #[test]
    fn transaction_output_exposes_lock_and_amount() {
        let owner = Owner::new_sig();
        let lock = owner.as_lock();
        let amount = WU::GDC();
        let output = TxOpt::new(lock, amount);

        assert_eq!(output.lock(), lock);
        assert_eq!(output.amount(), amount);
    }

    // Transaction inputs should retain the referenced transaction and output index.
    #[test]
    fn transaction_input_exposes_reference_and_index() {
        let tx_id = [7u8; 32];
        let input = TxOptRef::new(tx_id, 3);

        assert_eq!(input.tx(), &tx_id);
        assert_eq!(input.index(), 3);
    }
}