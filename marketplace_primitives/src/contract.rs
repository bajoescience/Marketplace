mod tx;
mod work;
mod result;
mod whiteroom;

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::{format, vec};

use crate::BlockHeader;
pub use crate::contract::work::{WorkPtr};
pub use crate::contract::result::ResultPtr;
use crate::{Verify};

pub use self::tx::{Tx, TxIdentifier, TxIO};
pub use self::whiteroom::{Whiteroom, WRVote};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::functions;
use marketplace_helpers::objects::{AgentResult, ID, IdHash, VRF_T, WU};
use marketplace_wallet::{Key, Lock};
use marketplace_wallet::crypto::Crypto;

// Contract type for a job
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum Contract {
    JOB(JobContract),
    TX(TxContract)
}

impl Contract {
    // pub fn as_tx(&self) -> impl IntoIterator<Item = Tx> {
    //     match self {
    //         Self::JOB(jobctr) => {
    //             jobctr.get_tx()
    //         },
    //         Self::TX(txctr) => {
    //             txctr.owned_tx()
    //         }
    //     }
    // }

    // Genesis Contract
    pub fn genesis() -> Self {
        Self::TX(TxContract::genesis())
    }

    // Calculate total new coins in a tx
    pub fn new_coins(&self) -> WU {
        if let Self::JOB(jobctr) = self {
            jobctr.new_gdc()
        }else {
            WU::default()
        }
    }
}

// Contract type for a job
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct JobContract {
    // Input block header
    blk_hdr: BlockHeader,
    input: WorkPtr, 
    output: Whiteroom<ResultPtr>,
}

impl JobContract {
    pub fn new(input: WorkPtr, blk_hdr: BlockHeader) -> Self {
        Self {
            blk_hdr,
            input,
            output: Whiteroom::new(),
        }
    }
}

// Getter methods
impl JobContract {
    // Work id
    pub fn work_id(&self) -> ID {
        self.input.id()
    }
    
    // Get VRF threshold of Job
    // using Blockheader
    pub fn vrf_t(&self) -> VRF_T {
        functions::get_vrf(
            self.blk_hdr.average(), 
            self.input.work_size().into()
        )
    }

    // Get whiteroom length
    pub fn wr_len(&self) -> usize {
        self.output.len()
    }

    // Get the blockheader ID inputed into the JobContract
    pub fn get_input_blk_hdr(&self) -> &BlockHeader {
        &self.blk_hdr
    }

    // Get workptr
    pub fn input(&self) -> &WorkPtr {
        &self.input
    }

    // Get whiteroom
    pub fn output(&self) -> &Whiteroom<ResultPtr> {
        &self.output
    }

    // Check if block header is correct
    pub fn is_same_blk_hdr(&self, blk_hdr: &BlockHeader) -> bool {
        if *self.input().blk_hdr() == blk_hdr.id() {
            return true;
        }

        false
    }

    // Create transactions for results in Contract
    pub fn get_tx(&self) -> Tx {
        Tx::from_contract(0, self)
    }

    // Get first result pointer
    pub fn first_result(&self) -> Option<&ResultPtr> {
        self.output.members().next()
    }
}

// Setter methods
impl JobContract {
    /// Validate all results in whiteroom at once
    pub fn validate_all(&self) -> AgentResult<()> {
        for result in self.output.members() {
            self.validate_result(result)?;
        }

        Ok(())
    }

    /// Newly printed gdc
    /// for every Whiteroom member, new gdc is printed
    /// But because the initial job price only covers one whiteroom memeber
    /// the final amount of gdc created is: 
    /// (Sum of Whiteroom pay) - Job pay
    pub fn new_gdc(&self) -> WU {
        // Sum
        let wr_pay_sum: WU = self.output.members()
            .map(|resptr| resptr.result().spent().into())
            .sum();
        
        wr_pay_sum - self.input.work_size().into()
    }

    /// Before attempting to validate a result
    /// The appropiate BlockHeader input to the WorkPtr
    ///  instance referenced by Self must be known
    /// To verify the result accordingly
    // Validate each result
    pub fn validate_result(&self, result: &ResultPtr) -> AgentResult<()> {
        let workptr = self.input();

        // Verify that result references workptr
        if result.work_id() != workptr.id() {
            return Err(format!(
                "Error: Invalid WorkPtr Reference"
            ))
        }

        // Ensure input work size is more than output work size
        if result.result().spent().into() > self.input.work_size().into() {
            return Err(format!(
                "Error: Result must spend less than commites work pay."
            ))
        } 

        // Verify result WRProof
        let vrf_t = self.vrf_t();

        let seed = functions::wr_seed(
            &workptr.id(), 
            result.wr_proof().pk_bytes(),
        );

        let diff = functions::vdf_difficulty(
            workptr.work_size().into()
        );

        let vrf_result = Crypto::wr_prove(result.wr_proof(), &seed, diff)?;

        if vrf_result > vrf_t {
            return Err(format!(
                "Error: Invalid Whiteroom Member"
            ))
        }

        // Verify result
        result.verify()
    }

    // Add result to JobContract
    pub fn add_result(&mut self, result: ResultPtr) -> AgentResult<usize> {
        // Validate result with reference to JobContract
        self.validate_result(&result)?;

        // Attempt to add to whiteroom
        self.output.add_member(result)
    }
}

impl Verify for JobContract {
    fn verify(&self) -> AgentResult<()> {
        // INPUTS
        // Verify work input
        self.input.verify()?;

        // OUTPUTS
        // Ensure that whiteroom has reached consensus
        if !self.output.is_consensus() {
            return Err(format!(
                "Error: Attempting to finalize invalid Whiteroom Consensus" 
            ))
        }

        // Validate all result pointers in contract
        self.validate_all()
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
// Contract type for casual transaction
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
mod tests {
    use std::{assert_eq, thread, vec};

use marketplace_helpers::{functions::{dum_bytes, vdf_difficulty, wr_seed}, objects::{IdHash, WU, WorkSize, WHITEROOM_MAX}};
use marketplace_wallet::{Owner, crypto::Crypto};

use crate::contract::{result::ResultInfo};

use super::*;

    // Work price
    // returns (Total amount, Work amount)
    fn work_pay() -> (WorkSize, WorkSize) {
        (
            WorkSize::build(WU::try_from(50000).unwrap()).unwrap() ,
            WorkSize::build(WU::try_from(10000).unwrap()).unwrap()
        )
    }

    // BlockHeader
    fn blk_hdr() -> BlockHeader {
        BlockHeader::genesis()
    }
    
    // WorkPtr
    fn workptr(owner: &Owner, blk_hdr: ID) -> WorkPtr {
        let mut workptr = WorkPtr::new(
            dum_bytes(), 
            work_pay().1,
            owner.as_wr_lock(),
            blk_hdr, 
            work::ExecuteTime::NOW
        );

        // Sign workptr
        let key = owner.sign(&workptr.id());
        workptr.add_auth(
            owner.as_wr_lock(), 
            key, 
            false
        ).unwrap();

        workptr
    }

    // ResultPtr
    fn resultptr(work_id: ID, wr_owner: &Owner) -> ResultPtr {
        // Get whiteroom proof
        let wr_proof = Crypto::new(&wr_owner)
            .attempt_wr(
                &wr_seed(
                    &work_id, 
                    &wr_owner.pk()
                ),
                vdf_difficulty(
                    work_pay().1.into()
                )
            ).unwrap();

        let result = ResultInfo::new(dum_bytes(), work_pay().1);
        
        let mut resptr = ResultPtr::new(work_id, result, wr_proof, dum_bytes(), 
            wr_owner.as_wr_lock()
        );

        // Prove ownership
        let key = wr_owner.sign(&resptr.id());

        resptr.add_auth(wr_owner.as_wr_lock(), key, false).unwrap();

        resptr
    }

    // Add a result to contract
    #[test]
    fn add_to_contract() -> AgentResult<()> {
        let ipt_owner = Owner::new_sig();
        let wr_owner = Owner::new_sig();

        // Block header
        let blk_hdr = blk_hdr();

        // Input
        let workptr = workptr(&ipt_owner, blk_hdr.id());
        let mut ctr = JobContract::new(workptr, blk_hdr);

        // Result
        let result_ptr = resultptr(
            ctr.input().id(),
            &wr_owner,
        );

        let len = ctr.add_result(result_ptr)?;
        assert_eq!(len, 1);

        // Verify contract but this should fail because
        // Whiteroom is invalid
        assert!(ctr.verify().is_err());

        Ok(())
    }

    // Verify contract
    #[test]
    fn add_to_contracts() -> AgentResult<()> {
        let ipt_owner = Owner::new_sig();

        // Block header
        let blk_hdr = blk_hdr();

        // Input
        let workptr = workptr(&ipt_owner, blk_hdr.id());
        let mut ctr = JobContract::new(workptr, blk_hdr);

        // ResultPtr Initializers
        let work_id = ctr.input().id();

        let mut handles = Vec::new();

        // Assemble Whiteroom max results
        for _ in 0..WHITEROOM_MAX {
            handles.push(thread::spawn(move || {
                let wr_owner = Owner::new_sig();

                // Result
                let result_ptr = resultptr(
                    work_id,
                    &wr_owner
                );

                result_ptr
            })); 
        }

        for handle in handles {
            let result_ptr = handle.join().unwrap();
            ctr.add_result(result_ptr)?;
        }

        // Total amount should be whiteroom max size
        assert_eq!(ctr.output.len(), WHITEROOM_MAX);

        // Total gdc added to the system through block
        assert_eq!(
            ctr.new_gdc(), 
            WU::try_from((WHITEROOM_MAX as u128 - 1) * 10000).unwrap()
        );

        // Contract should be validated and verified
        ctr.validate_all()?;
        ctr.verify()
    }

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