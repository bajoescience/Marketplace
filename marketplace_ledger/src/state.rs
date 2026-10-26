//! # State
//! 
//! A module that handles 
//! storing, and updating 
//! UTXO state

use std::{collections::{HashMap}, format};

use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, WU}};
use marketplace_primitives::{Tx, tx::{TxOpt, TxOptRef}, TxIdentifier};

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
    fn get(&mut self, utxo_ref: &TxOptRef) -> Option<TxOpt>;

    /// Put a UTXO into the balance sheet
    /// given the UTXO reference
    fn put(&mut self, utxo_ref: &TxOptRef, utxo: TxOpt);
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

    fn get(&mut self, utxo_ref: &TxOptRef) -> Option<TxOpt> {
        self.list.remove(&utxo_ref.id())
    }

    fn put(&mut self, utxo_ref: &TxOptRef, utxo: TxOpt) {
        self.list.insert(utxo_ref.id(), utxo);
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
    // Hashmap<token acc_id, (Balances, initial tokens)>
    coin: B,
    tokens: HashMap<ID, (B, WU)>,
}

impl<B: Balances> State<B> {
    pub fn new() -> Self {
        Self {
            coin: B::new(),
            tokens: HashMap::new(),
        }
    }

    fn get_balances(&mut self, tx: &Tx) -> AgentResult<&mut B> {
        // Check the appropiate token
        let balances = match tx.tid() {
            TxIdentifier::COIN => {
                &mut self.coin       
            },
            TxIdentifier::TOKEN(tid) => {
                let tid = tid.unwrap();

                // If token does not exist, but is not a new token
                // reject the token. 
                if !self.tokens.contains_key(&tid) && tx.acc_id() != tid {
                    return Err(format!(
                        "Error: Token {} does not exist",
                        functions::from_bytes(&tid)
                    ))
                }

                // Get token
                let token = self.tokens
                    .entry(tid)
                    .or_insert((B::new(), tx.opt_sum()));

                &mut token.0
            }
        };

        Ok(balances)
    }
}

impl<B: Balances> State<B> {
    /// Attempt to remove UTXO from state
    pub fn get_ref_opts<'a>(&mut self, tx: &'a Tx) -> AgentResult<Vec<(&'a TxOptRef, TxOpt)>> {
        let balances = self.get_balances(tx)?;

        // Keep track of used TxIO
        let mut ref_opts = Vec::new();

        // Get Tx outputs referenced by inputs in transaction
        for ipt in tx.ipts().iter() {
            let Some(ref_opt) = balances.get(ipt) else {
                // If reference utxo could not be found, return
                // all removed UTXO's and return error
                for (ipt, utxo) in ref_opts.into_iter().rev() {
                    balances.put(ipt, utxo);
                }

                return Err(format!(
                    "Error: UTXO addressed by {} could not be found in Tx {}",
                    functions::from_bytes(&ipt.id()),
                    functions::from_bytes(&tx.id())
                ))
            };

            ref_opts.push((ipt, ref_opt));
        }

        Ok(ref_opts)
    }

    // Revert outputs gotten from state given any error
    pub fn revert_ref_opts(&mut self, tx: &Tx, ref_opts: Vec<(&TxOptRef, TxOpt)>) {
        // Balances should already exists as we need to add input 
        // before adding outputs
        let balances = self.get_balances(tx)
            .expect("Illegal: Token should be created by inputs first before outputs");

        // Add each output to the corresponding
        // owner's balance
        for (ipt, opt) in ref_opts.into_iter().rev() {
            balances.put(ipt, opt);
        }
    }

    // Add utxo to state
    pub fn put_opts(&mut self, tx: &Tx) {
        let balances = self.get_balances(tx)
            .expect("Illegal: Token should exist, but not found");

        // Add each input back to the state
        for (index, utxo) in tx.opts().iter().rev().enumerate() {
            let ipt = TxOptRef::new(tx.id(), index);
            balances.put(&ipt, utxo.clone());
        }
    }
}

#[cfg(test)]
mod tests {

use super::*;

    // Test addition and subtraction to State 

    // Test reverting of nodes
}