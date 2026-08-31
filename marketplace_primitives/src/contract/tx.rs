mod input;

use std::{collections::HashMap, format, panic, vec};

pub use input::TxIO;

use borsh::{BorshDeserialize, BorshSerialize};
use chrono::{TimeZone, Utc};
use crate::{JobContract, Verify, helpers::{functions::{self, fee_price}, objects::{AgentResult, ID, IdHash, WU}}};
use crate::wallet::{Lock, Key};

// Transaction identifier for different types of currencies
// Coin represents native goldcoin
// Token represents any other currency with it's own identification
// The token ID is it's account hash
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum TxIdentifier {
    COIN,
    TOKEN(Option<ID>),
}

/// Tx input sum must always equal output sum

// Contract Execution information
// The Contract object is ultimately validated by the state.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Tx {
    // tid: Token ID
    // type identifier of Coin/Token to be spent
    // A COIN value is goldcoin
    tid: TxIdentifier,
    // Version
    ver: u32,
    // coin inputs
    ipts: Vec<TxIO>,
    // Coin ouputs
    opts: Vec<TxIO>,
    // Timestamp
    timestamp: u64,
}

impl PartialEq for Tx {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Tx {
    /// This associated function creates a new instance of transaction
    /// but the inputs and outputs are optional.
    /// 
    /// In that case, both inputs and outputs are represented as empty
    /// vectors.
    /// 
    /// # Example
    /// 
    /// ```
    /// use marketplace_primitives::{Tx, TxIdentifier};
    /// 
    /// let ver = 1;
    /// let tid = TxIdentifier::COIN;
    /// 
    /// let tx = Tx::new(ver, tid, None, None);
    /// 
    /// 
    /// assert!(tx.ipts().is_empty());
    /// assert!(tx.opts().is_empty());
    /// ```
    
    // Create a new contract with optional inputs and outputs
    pub fn new(
        ver: u32, 
        tid: TxIdentifier, 
        ipts: Option<Vec<TxIO>>,
        opts: Option<Vec<TxIO>>,
    ) -> Self {
        // tid is provided upfront and not validated
        // Because validation needs access to state
        // We'll later validate for each input,
        // it's source Contract must be of the same tid TxIdentifier as the new one
        // If even one input tid differs, transaction cannot be validated
        // Error is returned
        Self {
            ver,
            tid,
            ipts: ipts.unwrap_or_default(),
            opts: opts.unwrap_or_default(),
            timestamp: functions::timestamp(),
        }
    }

    /// This associated method is used
    /// to convert a Job contract into a transaction
    /// that sends the job price to whiteroom winning members
    /// 
    /// Any remainders encountered during errors it sends back
    /// to the job owner after the Tx instance has been created
    /// 
    
    // Create a new whiteroom transaction
    pub fn from_contract(ver: u32, ctr: &JobContract) -> Self {
        let employer = ctr.input().emp_wr();
        let work_size = ctr.input().work_size().into();

        // Inputs
        let ipts = vec![TxIO::new(
            *employer, 
            work_size
        )];

        // Outputs
        // Only winning whiteroom members receive the pay
        let mut opts = Vec::new();

        // Initalize amount spent using worksie
        let mut spent = work_size;
        let mut fees = WU::default();

        // New money is printed to send to all Whiteroom
        // winning witnesses
        for witness in ctr.output.winners() {
            spent = witness.result().spent().into();
            let fee = fee_price(spent);

            let wr_opt = TxIO::new(
                witness.witness(), 
                spent - fee
            );

            // Add fee to overall fees
            fees = fees + fee;

            opts.push(wr_opt);
        }

        // Add fees to outputs
        opts.push(TxIO::new(
            Lock::ACCOUNT(Self::gtt_tid()),
            fees
        ));

        // If spent is less than work size, send remainder back
        if spent < work_size {
            opts.push(TxIO::new(
                *employer,
                work_size - spent
            ));
        }

        Self {
            tid: TxIdentifier::COIN,
            ver,
            ipts,
            opts,
            timestamp: functions::timestamp(),
        }

    }

    /// Create a whiteroom tx instance in case of an error encountered
    /// while executing the work.
    /// 
    /// Unlike the "whiteroom_tx" associated method, this method sends
    /// some remainder back to the user
    
    // TODO: Create a whiteroom_tx for problematic work


    /// New account tokens have no inputs as they are 
    /// a start of a new currency.
    /// But this token initializer tx must be unique
    
    // Create a new account/ Token
    pub fn new_acc(ver: u32, opts: Option<Vec<TxIO>>) -> Self {
        let mut acc = Self {
            tid: TxIdentifier::TOKEN(None),
            ver,
            ipts: Vec::new(),
            opts: opts.unwrap_or_default(),
            timestamp: functions::timestamp(),
        };

        let tid = acc.id();

        // Panicing should be impossible as it only
        // panics when the tid is not a Token
        acc.set_tid(tid)
            .expect("Tx is clearly a token");

        acc
    }

    // Initialize genesis state for goldcoin
    pub fn genesis() -> Self {
        // TODO: Belongs to founder 
        let opts = vec![TxIO::new( 
            Lock::SIG(functions::founders()),
            WU::try_from(300 * 10u128.pow(12)).unwrap(),
        )];

        // This timestamp is a constant
        let dt = Utc.with_ymd_and_hms(
            2027, 1, 1, 0, 0, 0
        ).unwrap();

        Self {
            ver: 0,
            tid: TxIdentifier::COIN,
            ipts: vec![],
            opts,
            timestamp: dt.timestamp() as u64,
        }
    }

    // GTT token
    pub fn gtt() -> Self {
        // TODO: Belongs to founder 
        let opts = vec![TxIO::new( 
            Lock::SIG(functions::founders()),
            WU::try_from(10u128.pow(20)).unwrap(),
        )];

        // This timestamp is a constant
        let dt = Utc.with_ymd_and_hms(
            2027, 1, 1, 0, 0, 0
        ).unwrap();

        let mut gtt = Self {
            ver: 0,
            tid: TxIdentifier::TOKEN(None),
            ipts: vec![],
            opts,
            timestamp: dt.timestamp() as u64,
        };

        let tid = gtt.acc_id();

        // Panicking should be impossible
        gtt.set_tid(tid).unwrap();
        gtt
    }

    // Unlock tx by proving ownership of each input
    pub fn unlock(&self, auth: &HashMap<ID, Key>, msg_id: &ID) -> AgentResult<()> {
        for (index, ipt) in self.ipts().iter().enumerate() {
            // Search for key in auth
            let Some(key) = auth.get(&ipt.lock().id()) else {
                return Err(format!(
                    "Error: Key does not exist for input {} at inputs index {}",
                    functions::from_bytes(&ipt.id()), 
                    index
                ));
            };

            ipt.unlock(key, msg_id)?;
        }

        Ok(())
    }
}


// Getter functions
impl Tx {
    // Get Tokenid
    pub fn tid(&self) -> &TxIdentifier {
        &self.tid
    }

    // Get GTT tid
    pub fn gtt_tid() -> ID {
        let TxIdentifier::TOKEN(Some(tid)) = Self::gtt().tid else {
            panic!("Illegal: Gtt is a token and has ID");
        };

        tid
    }

    // Return list of all inputs
    pub fn ipts(&self) -> &Vec<TxIO> {
        &self.ipts
    }

    // Return list of all outputs
    pub fn opts(&self) -> &Vec<TxIO> {
        &self.opts
    }

    // Get fee output (always the first output)
    // Only for goldcoin contracts
    pub fn first_opt(&self) -> Option<&TxIO> {
        self.opts().get(0)
    }

    // Return sum of all inputs
    pub fn ipt_sum(&self) -> WU {
        self.ipts()
            .iter()
            .map(|opt| opt.amount())
            .sum()
    }

    // Return sum of outputs
    pub fn opt_sum(&self) -> WU {
        self.opts()
            .iter()
            .map(|opt| opt.amount())
            .sum()
    }

    // Return fees if it exists
    pub fn fees(&self) -> WU {
        self.opts()
            .iter()
            .map(|opt| 
                if Lock::ACCOUNT(Self::gtt_tid()) != *opt.lock() {
                    WU::default()
                } else {
                    opt.amount()
                }
            ).sum()
    }

    // Check if tx is a coin tx
    pub fn is_coin(&self) -> bool {
        if let TxIdentifier::COIN = self.tid() {
            true
        } else {
            false
        }
    }

    // Get new account id
    pub fn acc_id(&self) -> ID {
        // Create a new contract
        // everything is the same 
        // as the contract except for
        // an empty tid
        // and empty inputs
        let contract = Self {
            tid: TxIdentifier::TOKEN(None),
            ver: self.ver,
            ipts: Vec::new(),
            opts: self.opts.clone(),
            timestamp: self.timestamp,
        };

        // This becomes the new account id
        contract.id()
    }

    // Check if tx is a new account
    pub fn is_new_acc(&self) -> bool {
        // New account
        let TxIdentifier::TOKEN(Some(acc_id)) = self.tid else {
            return false;
        };

        self.acc_id() == acc_id
    }
}

// Setter functions
impl Tx {
    // Set Token ID
    fn set_tid(&mut self, tid: ID) -> AgentResult<()> {
        if let TxIdentifier::COIN = self.tid {
            return Err(format!("Cannot change ID for goldcoin tx"));
        } else {
            self.tid = TxIdentifier::TOKEN(Some(tid));
            Ok(())
        }
    }

    /// Add all inputs to the transaction
    /// But each input received with it's Owner
    /// So as to sign the resultant transaction 
    /// afterward
    
    // Add each input to contract
    pub fn add_ipts(&mut self, ipts: Vec<TxIO>) {
        self.ipts.extend(ipts);
    }

    // Add output to a contract
    pub fn add_opt(&mut self, opt: TxIO) {
        self.opts.push(opt);
    }
}

impl Verify for Tx {
    fn verify(&self) -> AgentResult<()> {
        match self.tid() {

            // Goldcoin tx
            TxIdentifier::COIN => {
                if self.ipts().is_empty() {
                    return Err(format!(
                        "Error: Goldcoin transactions must contain inputs"
                    ));
                }
            },

            // Token tx
            TxIdentifier::TOKEN(acc_id) => {
                if let Some(_) = acc_id {
                    // Check if ipts is empty
                    // Only new accounts input is empty
                    if self.is_new_acc() {
                        if !self.ipts().is_empty() {
                            return Err(format!(
                                "Error: Account transactions must not contain inputs"
                            ));
                        }

                        // Ensure tx has outputs
                        if self.opts().is_empty() {
                            return Err(format!("Error: Tx must contain outputs"));
                        }

                        return Ok(());
                    } else {
                        if self.ipts().is_empty() {
                            return Err(format!(
                                "Error: Transactions must contain inputs"
                            ));
                        }
                    }
                } else {
                    return Err(format!("Error: Tx Token ID not found"));
                };
            },
        }

        // Ensure tx has outputs
        if self.opts().is_empty() {
            return Err(format!("Error: Tx must contain outputs"));
        }

        // Ensure sum of outputs and inputs is zero
        if self.opt_sum() != self.ipt_sum() {
            return Err(format!(
                "Error: Tx input and output sum are not equal"
            ))
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{assert_eq, panic, vec};

use marketplace_helpers::{functions::dum_bytes};
use marketplace_wallet::{Owner};

use super::*;

    // Create a verifiable tx
    // Select strings to exclude some fields
    // for testing
    fn verify_coin_tx(exclude: &str) -> AgentResult<Tx> {
        let owner = Owner::new_sig();

        let lock = owner.as_lock();

        // Send coin from owner to owner
        let ipt = TxIO::new( 
            lock,
            WU::try_from(20000).unwrap()
        );

        let opt = TxIO::new(lock, WU::try_from(20000).unwrap());

        let opts = Some(vec![opt]);

        // Select appropiate tx based on test string
        let mut tx = match exclude {
            "false_id" => Tx::new(1, TxIdentifier::TOKEN(Some(dum_bytes())), None, opts),
            "tid" => Tx::new(1, TxIdentifier::TOKEN(None), None,  opts),
            "ipts" => Tx::new(1, TxIdentifier::COIN, None, opts),
            "opts" => Tx::new(1, TxIdentifier::COIN, None, None),
            _ => Tx::new(1, TxIdentifier::COIN, None, opts)
        };

        match exclude {
            "false_id" | "ipts" => Ok(tx),
            _ => {
                tx.add_ipts(vec![ipt]);
                Ok(tx)
            }
        }

    }

    // Verify tx with no outputs
    // should fail
    #[test]
    fn verify_tx_no_opts() {
        let tx = verify_coin_tx("opts").unwrap();
        let err = tx.verify().unwrap_err();
        assert_eq!(err, "Error: Tx must contain outputs")
    }

    // Tx with false token id and no inputs 
    // should fail
    #[test]
    fn verify_tx_false_id_no_inputs() {
        let tx = verify_coin_tx("false_id").unwrap();
        let err = tx.verify().unwrap_err();
        assert_eq!(err, "Error: Transactions must contain inputs");
    }

    // Verify tx but with no token id
    // should fail
    #[test]
    fn verify_tx_no_tid() {
        let tx = verify_coin_tx("tid").unwrap();
        let err = tx.verify().unwrap_err();
        assert_eq!(err, "Error: Tx Token ID not found");
    }

    // Verify tx but with token id
    // should pass
    #[test]
    fn verify_tx_with_tid() -> AgentResult<()> {
        let owner = Owner::new_sig();

        // Account output
        let opt = TxIO::new(
            owner.as_lock(), 
            WU::try_from(20000).unwrap()
        );

        let tx = Tx::new_acc(0, Some(vec![opt]));

        // Confirm it is a new account
        assert!(tx.is_new_acc());

        tx.verify()
    }

    // Verify coin tx but with no inputs
    // shoulf fail
    #[test]
    fn verify_tx_no_ipts() {
        let tx = verify_coin_tx("ipts").unwrap();
        let err = tx.verify().unwrap_err();

        assert_eq!(err, "Error: Goldcoin transactions must contain inputs")
    }

    // Verify tx
    #[test]
    fn verify_full_tx() {
        let tx = verify_coin_tx("").unwrap();
        assert!(tx.verify().is_ok())
    }

    // Unlock a transaction
    #[test]
    fn unlock_tx() -> AgentResult<()> {
        let owner = Owner::new_sig();
        let lock = owner.as_lock();

        let ipt = TxIO::new(
            lock,
            WU::try_from(20000).unwrap()
        );

        let ipts = vec![ipt];

        let tx = Tx::new(1, TxIdentifier::COIN, Some(ipts), None);

        // Create a test auth to hold lock and key for each input
        let mut auth = HashMap::new();

        auth.insert(lock.id(), owner.sign(&tx.id()));

        assert!(tx.unlock(&auth, &tx.id()).is_ok());

        Ok(())
    }

    // Create a new token and 
    // Verify token ID
    #[test]
    fn new_acc_tx() {
        let acc = Tx::new_acc(2, None);

        let TxIdentifier::TOKEN(id) = acc.tid() else {
            panic!("Contract type must be token")
        };

        assert_eq!(id.unwrap(), acc.acc_id());
    }

    // Test genesis function
    #[test]
    fn genesis_tx() {
        let ctr = Tx::genesis();

        let opt = ctr.opts.get(0).unwrap();

        assert_eq!(opt.amount(), WU::try_from(300000000000000).unwrap());
    }

    // Add input to tx
    #[test]
    fn add_input_to_tx() -> AgentResult<()> {
        let owner = Owner::new_sig();

        let mut tx = Tx::new(
            2, 
            TxIdentifier::COIN, 
            None,
            None
        );

        let ipt = TxIO::new(
            owner.as_lock(),
            WU::try_from(200000).unwrap()
        );

        // Add input to tx
        tx.add_ipts(vec![ipt]);

        assert_eq!(tx.ipts().len(), 1);

        Ok(())
    }
}
