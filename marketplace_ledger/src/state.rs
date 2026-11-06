//! # State
//! 
//! A module that handles 
//! storing, and updating 
//! UTXO state

use std::{collections::HashMap, format};

use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, WU}};
use marketplace_primitives::par_exec::{Tx, TxIdentifier, TxOpt, TxOptRef};
use marketplace_wallet::{Key, Lock};

// Helper function confirming utxo ownership
pub fn confirm_utxo(
    utxo: &TxOpt,
    msg_id: &ID, 
    auth: &HashMap<ID, Key>
) -> AgentResult<()> {
    // Ensure utxo owner consented
    let owner = utxo.lock();
            
    let Some(key) = auth.get(&owner.id()) else {
        return Err(format!(
            "Error: Tx Output {} not found.",
            functions::from_bytes(&utxo.id())
        ))
    };

    owner.unlock(key, msg_id)
}

// Helper functin to revert utxo
pub fn revert_utxo<B: Balances>(ref_opts: Vec<(TxOptRef, TxOpt)>, balances: &mut B) {
    for (ipt, utxo) in ref_opts.into_iter().rev() {
        balances.put(ipt, utxo);
    }
}

/// Balances trait defines a set of methods
/// that use a custom data structure to handle the adding 
/// and removing of UTXOs to the Balance sheet for each Token type.
pub trait Balances: Clone {
    /// Create a new balance instance to be used
    /// for every new token to store it's UTXO state
    fn new() -> Self;

    /// Get a UTXO using a reference and the Token type
    /// which returns a UTXO Option. 
    /// 
    /// The token type is necessary because only utxo of a token type can
    /// exist in the same transaction, and it is up to the implemntor of the trait
    /// to ensure this is always the case.
    fn get(&mut self, utxo_ref: TxOptRef) -> Option<TxOpt>;

    /// Put a UTXO into the balance sheet
    /// given the UTXO reference
    fn put(&mut self, utxo_ref: TxOptRef, utxo: TxOpt);

    /// Get the balance of an owner in WU
    /// given the owner's lock form
    fn balance(&self, owner: Lock) -> WU;
}

/// Balances is a struct containing all
/// the public key hashes and the amount of the currency being held
#[derive(Clone)]
pub struct DefaultBalances {
    list: HashMap<ID, TxOpt>
}

impl Balances for DefaultBalances {
    fn new() -> Self {
        Self {
            list: HashMap::new(),
        }
    }

    fn get(&mut self, utxo_ref: TxOptRef) -> Option<TxOpt> {
        self.list.remove(&utxo_ref.id())
    }

    fn put(&mut self, utxo_ref: TxOptRef, utxo: TxOpt) {
        self.list.insert(utxo_ref.id(), utxo);
    }

    // The owner's balances list could be stored seperately
    fn balance(&self, owner: Lock) -> WU {
        let mut balance = WU::default();

        // Iterate through list and find utxo's owned by 
        // owner
        for (_, utxo) in self.list.iter() {
            if utxo.lock() == owner {
                balance = balance + utxo.amount();
            }
        }
        balance
    }
}

// Getter methods
impl DefaultBalances {
    pub fn len(&self) -> usize {
        self.list.len()
    }
}

/// A state is the summary of all financial history in memory
/// 
/// The state keeps all Tokens on the first layer, and then keeps all 
/// Token Balances in the second layer
#[derive(Clone)]
pub struct State<B: Balances> {
    coin: B,

    // Hashmap<token acc_id, (Balances, initial tokens)>
    tokens: HashMap<ID, (B, WU)>,
}

impl<B: Balances> State<B> {
    pub fn new() -> Self {
        Self {
            coin: B::new(),
            tokens: HashMap::new(),
        }
    }

    /// Get goldcoin balance sheet
    pub fn get_mut_coin_balance_list(&mut self) -> &mut B {
        &mut self.coin
    }

    /// Get token balance sheet and total amount of token available
    pub fn get_mut_token_balance_list(&mut self, tid: ID) -> Option<&mut B> {
        self.tokens
            .get_mut(&tid)
            .map(|(balance_list, _)| balance_list)
    }

    pub fn get_tx_balances(&mut self, tx: &Tx) -> AgentResult<&mut B> {
        let balances = match tx.tid() {
            TxIdentifier::COIN => {
                &mut self.coin
            },
            TxIdentifier::TOKEN(tid) => {
                let tid = tid.expect("Illegal: Token ID must exist");

                if tx.acc_id() == tid {  
                    &mut self.tokens
                        .entry(tid).or_insert((B::new(), tx.opt_sum())).0
                } else {
                    return Err(format!(
                        "Error: Token {} does not exist",
                        functions::from_bytes(&tid)
                    ));
                }
            }
        };

        Ok(balances)
    } 

    /// Get gdc balance of an owner
    pub fn get_gdc_balance(&self, owner: Lock) -> WU {
        self.coin.balance(owner)
    }

    /// Get token balance of an owner
    /// using the token id
    /// 
    /// Returns Err if token does not exist
    pub fn get_token_balance(&self, tid: ID, owner: Lock) -> AgentResult<WU> {
        let Some(list) = self.tokens.get(&tid) else {
            return Err(format!(
                "Error: Token does not exist"
            ))
        };

        Ok(list.0.balance(owner))
    }
}

impl<B: Balances> State<B> {
    /// Attempt to remove UTXO from state
    pub fn get_ref_opts(&mut self, tx: &Tx, msg_id: &ID, auth: &HashMap<ID, Key>) -> AgentResult<Vec<(TxOptRef, TxOpt)>> {
        let balances = self.get_tx_balances(tx)?;

        // Keep track of used TxIO
        let mut ref_opts = Vec::new();

        // Get Tx outputs referenced by inputs in transaction
        for ipt in tx.ipts().iter() {
            let Some(opt) = balances.get(*ipt) else {
                // If reference utxo could not be found, return
                // all removed UTXO's and return error
                revert_utxo(ref_opts, balances);

                return Err(format!(
                    "Error: UTXO addressed by {} could not be found in Tx {}",
                    functions::from_bytes(&ipt.id()),
                    functions::from_bytes(&tx.id())
                ))
            };

            // Confirm output is signed by owner
            confirm_utxo(&opt, msg_id, auth)?;

            ref_opts.push((*ipt, opt));
        }

        Ok(ref_opts)
    }

    // Revert outputs gotten from state given any error
    pub fn revert_ref_opts(&mut self, tx: &Tx, ref_opts: Vec<(TxOptRef, TxOpt)>) {
        // Balances should already exists as we need to add input 
        // before adding outputs
        let balances = self.get_tx_balances(tx)
            .expect("Illegal: Token should be created by inputs first before outputs");

        // Add each output to the corresponding
        // owner's balance
        for (ipt, opt) in ref_opts.into_iter().rev() {
            balances.put(ipt.clone(), opt);
        }
    }

    // Add utxo to state
    pub fn put_opts(&mut self, tx: &Tx) {
        let balances = self.get_tx_balances(tx)
            .expect("Illegal: Token should exist, but not found");

        // Add each input back to the state
        for (index, utxo) in tx.opts().iter().rev().enumerate() {
            let ipt = TxOptRef::new(tx.id(), index);
            balances.put(ipt, utxo.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use marketplace_helpers::objects::{IdHash, WU};
    use marketplace_primitives::par_exec::{Tx, TxIdentifier, TxOpt, TxOptRef};
    use marketplace_wallet::Owner;

    use super::*;

    // DefaultBalances should store, retrieve, remove, and sum UTXOs for an owner.
    #[test]
    fn default_balances_store_and_remove_utxos() {
        let owner = Owner::new_sig();
        let lock = owner.as_lock();
        let reference = TxOptRef::new([1; 32], 0);
        let amount = WU::try_from(17_000).unwrap();
        let output = TxOpt::new(lock, amount);
        let mut balances = DefaultBalances::new();

        balances.put(reference, output);
        assert_eq!(balances.len(), 1);
        assert_eq!(balances.balance(lock), amount);
        assert_eq!(balances.get(reference), Some(output));
        assert_eq!(balances.len(), 0);
        assert_eq!(balances.balance(lock), WU::default());
    }

    // State should place transaction outputs in the coin balance sheet by output index.
    #[test]
    fn putting_transaction_outputs_updates_coin_balance() {
        let owner = Owner::new_sig();
        let lock = owner.as_lock();
        let amount = WU::try_from(23_000).unwrap();
        let tx = Tx::new(
            1,
            TxIdentifier::COIN,
            None,
            Some(vec![TxOpt::new(lock, amount)]),
        );
        let mut state = State::<DefaultBalances>::new();

        state.put_opts(&tx);

        assert_eq!(state.get_gdc_balance(lock), amount);
        let reference = TxOptRef::new(tx.id(), 0);
        assert_eq!(
            state.get_mut_coin_balance_list().get(reference),
            Some(TxOpt::new(lock, amount))
        );
    }

    // UTXO confirmation should accept the owner's signature over the spending transaction ID.
    #[test]
    fn confirms_utxo_with_owner_signature() {
        let owner = Owner::new_sig();
        let output = TxOpt::new(owner.as_lock(), WU::GDC());
        let message_id = [8; 32];
        let mut auth = HashMap::new();
        auth.insert(owner.as_lock().id(), owner.sign(&message_id));

        assert!(confirm_utxo(&output, &message_id, &auth).is_ok());
    }

    // UTXO confirmation should reject an output whose owner has no authentication entry.
    #[test]
    fn rejects_utxo_without_owner_signature() {
        let owner = Owner::new_sig();
        let output = TxOpt::new(owner.as_lock(), WU::GDC());

        assert!(confirm_utxo(&output, &[8; 32], &HashMap::new()).is_err());
    }

    // Removing transaction inputs should preserve enough data to restore them after a failure.
    #[test]
    fn get_and_revert_transaction_inputs_restores_utxos() -> AgentResult<()> {
        let owner = Owner::new_sig();
        let lock = owner.as_lock();
        let reference = TxOptRef::new([3; 32], 0);
        let output = TxOpt::new(lock, WU::try_from(12_000).unwrap());
        let tx = Tx::new(
            1,
            TxIdentifier::COIN,
            Some(vec![reference]),
            Some(vec![TxOpt::new(lock, output.amount())]),
        );
        let mut state = State::<DefaultBalances>::new();
        state.get_mut_coin_balance_list().put(reference, output);
        let mut auth = HashMap::new();
        auth.insert(lock.id(), owner.sign(&tx.id()));

        let removed = state.get_ref_opts(&tx, &tx.id(), &auth)?;
        assert_eq!(removed.len(), 1);
        assert_eq!(state.get_gdc_balance(lock), WU::default());

        state.revert_ref_opts(&tx, removed);
        assert_eq!(state.get_gdc_balance(lock), output.amount());
        Ok(())
    }

    // A token balance query should distinguish an unknown currency from an empty balance.
    #[test]
    fn unknown_token_balance_returns_an_error() {
        let owner = Owner::new_sig();
        let state = State::<DefaultBalances>::new();

        assert!(state.get_token_balance([4; 32], owner.as_lock()).is_err());
    }
}