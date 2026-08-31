//! # State
//! 
//! A module that handles 
//! storing, and updating 
//! UTXO state

use std::{collections::{HashMap}, format};

use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, WU}};
use marketplace_wallet::Lock;
use marketplace_primitives::{Tx, TxIO, TxIdentifier};

/// State trait defines a set of methods that allows the use 
/// of a custom Data Structure to handle the balance sheet of 
/// every single token
pub trait State: Clone {
    /// Update State by:
    /// 
    /// Substracting the amount in a TxIO from the owner's balance as a Tx input,
    /// 
    /// Return any double spend error and roll back changes.
    fn try_update_ipt(&mut self, tx: &Tx) -> AgentResult<()>;

    // Revert tx inputs if anything goes wrong
    fn revert_ipt(&mut self, tx: &Tx);

    /// Update opt
    /// by adding the tx output TxIO to state
    fn update_opt(&mut self, tx: &Tx);

    /// Revert tx ouputs if anything goes wrong
    /// Note: Error encountered reverting will cause
    /// this method to panic
    fn revert_opt(&mut self, tx: &Tx) -> AgentResult<()>;

    /// Update by subtracting Tx ipts,
    /// and adding Tx opts
    fn try_update(&mut self, tx: &Tx) -> AgentResult<()> {
        self.try_update_ipt(tx)?;

        self.update_opt(tx);

        Ok(())
    }

    /// Revert by adding Tx ipts,
    /// and subtracting Tx opts
    fn revert(&mut self, tx: &Tx) -> AgentResult<()> {
        self.revert_ipt(tx);

        self.revert_opt(tx)
    }

    /// Get owner's a lock goldcoin balance.
    fn get_gdc_balance(&self, owner: Lock) -> WU;

    /// Get owner's token balance
    /// and how much it is worth in gdc
    /// given by the return value 
    /// (Token amount, Worth in gdc)
    fn get_token_balance(&self, owner: Lock, tid: ID) -> AgentResult<TokenBalance>;
}

/// Token balance which returns
/// 
/// the type of token
/// 
/// the amount of tokens
/// 
/// the worth of those tokens in gdc
#[derive(Debug, Clone, Copy)]
pub struct TokenBalance {
    pub token: Lock,
    pub amount: WU,
    pub worth: WU,
}

/// Balances is a struct containing all
/// the public key hashes and the amount of the currency being held
#[derive(Clone)]
struct Balances {
    list: HashMap<ID, WU>
}

impl Balances {
    fn new() -> Self {
        Self {
            list: HashMap::new(),
        }
    }
}

// Getter methods
impl Balances {
    fn len(&self) -> usize {
        self.list.len()
    }

    fn balance_of(&self, lock: Lock) -> WU {
        if let Some(balance) = self.list.get(&lock.id()) {
            *balance
        } else {
            WU::default()
        }
    }
}

// Setter methods
impl Balances {
    pub fn add(&mut self, opt: &TxIO) {
        // Add owner to balance sheet if it does not exist
        let balance = self.list
            .entry(opt.lock().id())
            .or_insert(WU::default());

        *balance = *balance + opt.amount();
    }

    pub fn sub(&mut self, ipt: &TxIO) -> AgentResult<()> {
        let Some(balance) = self.list.get_mut(&ipt.lock().id()) else {
            return Err(format!(
                "Error: Account {} does not exist",
                functions::from_bytes(&ipt.lock().id())
            ))
        };

        if *balance < ipt.amount() {
            return Err(format!(
                "Error: Not enough gdc to perform operation: Take {}, Account {} remainder is {}",
                ipt.amount(),
                functions::from_bytes(&ipt.lock().id()),
                *balance
            ))
        } else {
            *balance = *balance - ipt.amount();
            Ok(())
        }
    }
}

/// A state is the summary of all financial history in memory
/// 
/// The state keeps all Tokens on the first layer, and then keeps all 
/// Token Balances in the second layer
#[derive(Clone)]
pub struct DefaultState {
    // Hashmap<token acc_id, (Balances, initial tokens)>
    coin: Balances,
    tokens: HashMap<ID, (Balances, WU)>,
}

impl DefaultState {
    pub fn new() -> Self {
        Self {
            coin: Balances::new(),
            tokens: HashMap::new(),
        }
    }

    fn get_balances(&mut self, tx: &Tx) -> AgentResult<&mut Balances> {
        // Check the appropiate token
        let balances = match tx.tid() {
            TxIdentifier::COIN => {
                &mut self.coin       
            },
            TxIdentifier::TOKEN(tid) => {
                let tid = tid.unwrap();

                // Check token first
                if !self.tokens.contains_key(&tid) && tx.acc_id() != tid {
                    return Err(format!(
                        "Error: Token {} does not exist",
                        functions::from_bytes(&tid)
                    ))
                }

                let token = self.tokens
                    .entry(tid)
                    .or_insert((Balances::new(), tx.opt_sum()));

                &mut token.0
            }
        };

        Ok(balances)
    }
}

impl State for DefaultState {
    // Attempt to remove
    fn try_update_ipt(&mut self, tx: &Tx) -> AgentResult<()> {
        let balances = self.get_balances(tx)?;

        // Keep track of used TxIO
        let mut imp = Vec::new();

        // Attempt to subtract each input from corresponding 
        // owner's balance
        for ipt in tx.ipts() {
            match balances.sub(ipt) {
                Ok(_) => imp.push(ipt),

                Err(e) => {
                    for ipt in imp {
                        balances.add(ipt)
                    }

                    return Err(e);
                }
            };
        }

        Ok(())
    }

    // Revert input given any error
    fn revert_ipt(&mut self, tx: &Tx) {
        let balances = self.get_balances(tx)
            .expect("Illegal: Token should exist, but not found");

        // Add each input back to the state
        for ipt in tx.ipts() {
            balances.add(ipt);
        }
    }

    // Update only tx output
    fn update_opt(&mut self, tx: &Tx) {
        // Balances should already exists as we need to add input 
        // before adding outputs
        let balances = self.get_balances(tx)
            .expect("Illegal: Token should be created by inputs first before outputs");

        // Add each output to the corresponding
        // owner's balance
        for opt in tx.opts() {
            balances.add(opt)
        }
    }

    // Revert output
    fn revert_opt(&mut self, tx: &Tx) -> AgentResult<()> {
        let balances = self.get_balances(tx)?;

        // Add each output to the corresponding
        // owner's balance
        for opt in tx.opts() {
            balances.sub(opt)?;
        }

        Ok(())
    }

    fn get_gdc_balance(&self, owner: Lock) -> WU {
        self.coin.balance_of(owner)
    }

    fn get_token_balance(&self, owner: Lock, tid: ID) -> AgentResult<TokenBalance> {
        let Some(acc) = self.tokens.get(&tid) else {
            return Err(format!(
                "Error: Token of id {} does not exist",
                functions::from_bytes(&tid)
            ));
        };

        // Get amount of tokens owned by lock
        let amount = acc.0.balance_of(owner);

        let acc_lock = Lock::acc(tid);

        // Worth of amount tokens in gdc
        let worth = (amount * self.coin.balance_of(acc_lock)) / acc.1;

        // Return token balance
        Ok(TokenBalance {
            token: owner,
            amount,
            worth,
        })
    }
}

#[cfg(test)]
mod tests {

    use std::{assert_eq, vec};

use marketplace_helpers::functions::dum_bytes;
use marketplace_wallet::Owner;

use super::*;

    #[test]
    fn operate_on_balances() {
        // Owner
        let owner = Owner::new_sig();

        let mut balances = Balances::new();

        // Add to balances
        let opt = TxIO::new(
            owner.as_lock(), 
            WU::try_from(30000).unwrap()
        );

        balances.add(&opt);
        assert_eq!(balances.list.len(), 1);

        // Remove from balances succesfully
        let ipt = TxIO::new(
            owner.as_lock(), 
            WU::try_from(20000).unwrap()
        );

        assert!(balances.sub(&ipt).is_ok());

        // Remove from balances should fail
        let ipt = TxIO::new(
            owner.as_lock(), 
            WU::try_from(20000).unwrap()
        );

        assert!(balances.sub(&ipt).is_err());

        // Remove from balances should work
        let ipt = TxIO::new(
            owner.as_lock(), 
            WU::try_from(10000).unwrap()
        );

        assert!(balances.sub(&ipt).is_ok());

    } 

    // Get gdc balance
    #[test]
    fn get_balance() -> AgentResult<()> {
        // State
        let mut state = DefaultState::new();

        // Transaction
        let owner = Owner::new_sig();

        // Tx output
        let balance = WU::try_from(30000).unwrap();
        let opt = TxIO::new(
            owner.as_lock(), 
            balance
        );

        let tx = Tx::new(0, TxIdentifier::COIN, None, Some(vec![opt]));

        // Update state
        state.try_update(&tx)?;

        // Get gdc balance
        assert_eq!(
            state.get_gdc_balance(owner.as_lock()), 
            balance
        );

        // Get token balance
        assert!(state.get_token_balance(owner.as_lock(), dum_bytes()).is_err());
        Ok(())

    }
}