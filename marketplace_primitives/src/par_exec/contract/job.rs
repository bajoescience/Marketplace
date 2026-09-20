use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, WU}};
use marketplace_wallet::crypto::Crypto;

use crate::{BlockHeader, ResultPtr, Tx, Verify, Whiteroom, WorkPtr};

/// ## Job Contract
/// 
/// A `Job Contract` is a type of contract that holds
/// all the information concerning a job
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

    // Get whiteroom length
    pub fn wr_len(&self) -> usize {
        self.output.len()
    }

    // Get the blockheader ID inputed into the JobContract
    pub fn get_blk_hdr(&self) -> &BlockHeader {
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
        let seed = functions::wr_seed(
            &workptr.id(), 
            result.wr_proof().pk_bytes(),
        );

        // Get difficulty using VRF_T and total work size
        // according to the Whitepaper (Page 4)
        let diff = self.get_blk_hdr().vdf_diff();

        let vrf_result = Crypto::wr_prove(result.wr_proof(), &seed, diff)?;

        if vrf_result > self.get_blk_hdr().vrf_t() {
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

#[cfg(test)]
pub mod tests {
    use std::{assert_eq, thread};

use marketplace_helpers::{functions::{dum_bytes, wr_seed}, objects::{IdHash, WHITEROOM_SIZE, WU, WorkSize}};
use marketplace_wallet::{Owner, crypto::Crypto};

use crate::{BlockHeader, par_exec::{ExecuteTime, ResultInfo}};

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
    pub fn workptr(owner: &Owner, blk_hdr: ID) -> WorkPtr {
        let mut workptr = WorkPtr::new(
            dum_bytes(), 
            work_pay().1,
            owner.as_wr_lock(),
            blk_hdr, 
            ExecuteTime::NOW
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
    pub fn resultptr(work_id: ID, wr_owner: &Owner, vdf_diff: u64) -> ResultPtr {
        // Get whiteroom proof
        let wr_proof = Crypto::new(&wr_owner)
            .attempt_wr(
                &wr_seed(
                    &work_id, 
                    &wr_owner.pk()
                ),
                vdf_diff
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
            blk_hdr.vdf_diff()
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
        for _ in 0..WHITEROOM_SIZE {
            handles.push(thread::spawn(move || {
                let wr_owner = Owner::new_sig();

                // Result
                let result_ptr = resultptr(
                    work_id,
                    &wr_owner,
                    blk_hdr.vdf_diff()
                );

                result_ptr
            })); 
        }

        for handle in handles {
            let result_ptr = handle.join().unwrap();
            ctr.add_result(result_ptr)?;
        }

        // Total amount should be whiteroom max size
        assert_eq!(ctr.output.len(), WHITEROOM_SIZE);

        // Total gdc added to the system through block
        assert_eq!(
            ctr.new_gdc(), 
            WU::try_from((WHITEROOM_SIZE as u128 - 1) * 10000).unwrap()
        );

        // Contract should be validated and verified
        ctr.validate_all()?;
        ctr.verify()
    }
}