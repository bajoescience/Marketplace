use std::collections::{HashMap, hash_map::Entry};

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, CONTRACT_WHITEROOM_SIZE, ID, IdHash, WU, WorkSize}};
use marketplace_wallet::{Key, Lock, crypto::Crypto};

use crate::{BlockHeader, ResultPtr, Tx, Verify, Whiteroom, WorkPtr};


/// Maximum whiteroom Size
const WHITEROOM_MAX: usize = functions::max_wr_size_from_expected(CONTRACT_WHITEROOM_SIZE);

/// ## Job Contract
/// 
/// A `Job Contract` is a type of contract that holds
/// all the information concerning a job
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct JobContract {
    // Input block header
    blk_hdr: BlockHeader,
    input: WorkPtr, 
    output: Whiteroom<ResultPtr, WHITEROOM_MAX>,

    // Authenticate JobContract
    // by locking workptr and
    // Stores <Lock.id(), Key>
    #[borsh(skip)]
    auth: HashMap<ID, Key>
}

impl JobContract {
    pub fn new(input: WorkPtr, blk_hdr: BlockHeader) -> Self {
        Self {
            blk_hdr,
            input,
            output: Whiteroom::new(),
            auth: HashMap::new(),
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
    pub fn output(&self) -> &Whiteroom<ResultPtr, WHITEROOM_MAX> {
        &self.output
    }

    /// Check if a job can reach consensus
    pub fn can_consensus(&self) -> bool {
        self.output().can_consensus()
    }

    /// Check if job has reached consensus
    pub fn is_consensus(&self) -> bool {
        self.output().is_consensus()
    }

    // Check if block header is correct
    pub fn is_same_blk_hdr(&self, blk_hdr: &BlockHeader) -> bool {
        if *self.input().blk_hdr() == blk_hdr.id() {
            return true;
        }

        false
    }

    // Create transactions for results in Contract
    pub fn get_tx(&self) -> AgentResult<Tx> {
        Tx::from_contract(0, self)
    }

    // Get first result pointer
    pub fn first_result(&self) -> Option<&ResultPtr> {
        self.output.members().next()
    }

    /// Get amount spent by winning whiteroom members
    /// 
    /// there may be no winning whiteroom result, in that case
    /// method will return Error.
    pub fn spent(&self) -> AgentResult<WorkSize> {
        self.output()
            .winners()
            .next()
            .map(
                |resultptr| 
                resultptr.result().spent()
            )
            .ok_or(
                format!("Err: Whiteroom has no winning node")
            )
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
        let wr_pay_sum: WU = self.output.winners()
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
                "Error: Result must not spend more than work pay."
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

impl Verify for JobContract {
    fn verify(&self) -> AgentResult<()> {
        // INPUTS
        // Ensure that 

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
    use std::{assert_eq, thread, vec};

use marketplace_helpers::{functions::{dum_bytes, wr_seed}, objects::{IdHash, WU, WorkSize}};
use marketplace_wallet::{Owner, crypto::Crypto};

use crate::{BlockHeader, par_exec::{ExecuteTime, ResultInfo}, sync_final::block::BlockBody, tx::TxOptRef};

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
        WorkPtr::new(
            dum_bytes(), 
            work_pay().1,
            vec![
                TxOptRef::new(
                    [255u8; 32], 
                    0
                )],
            owner.as_wr_lock(),
            blk_hdr, 
            ExecuteTime::NOW
        )
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
        let mut ctr = JobContract::new(workptr, blk_hdr.clone());

        // Sign workptr and add to ctr
        let key = ipt_owner.sign(&ctr.input().id());
        ctr.add_auth(
            ipt_owner.as_wr_lock(), 
            key, 
            false
        ).unwrap();

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
        let mut ctr = JobContract::new(workptr, blk_hdr.clone());

        // ResultPtr Initializers
        let work_id = ctr.input().id();

        let mut handles = Vec::new();

        // Assemble Whiteroom max results
        for _ in 0..CONTRACT_WHITEROOM_SIZE {
            let header = blk_hdr.clone();
            handles.push(thread::spawn(move || {
                let wr_owner = Owner::new_sig();

                // Result
                let result_ptr = resultptr(
                    work_id,
                    &wr_owner,
                    header.vdf_diff()
                );

                result_ptr
            })); 
        }

        for handle in handles {
            let result_ptr = handle.join().unwrap();
            ctr.add_result(result_ptr)?;
        }

        // Total amount should be whiteroom max size
        assert_eq!(ctr.output.len(), CONTRACT_WHITEROOM_SIZE);

        // Total gdc added to the system through block
        assert_eq!(
            ctr.new_gdc(), 
            WU::try_from((CONTRACT_WHITEROOM_SIZE as u128 - 1) * 10000).unwrap()
        );

        // Contract should be validated and verified
        ctr.validate_all()?;
        ctr.verify()
    }

    // A new job contract should expose its input and an empty result set.
    #[test]
    fn new_job_contract_exposes_input_and_empty_output() {
        let owner = Owner::new_sig();
        let block_header = blk_hdr();
        let work_ptr = workptr(&owner, block_header.id());
        let contract = JobContract::new(work_ptr.clone(), block_header.clone());

        assert_eq!(contract.work_id(), work_ptr.id());
        assert_eq!(contract.wr_len(), 0);
        assert_eq!(contract.input().id(), work_ptr.id());
        assert_eq!(contract.get_blk_hdr().id(), block_header.id());
        assert!(contract.output().members().next().is_none());
        assert!(contract.first_result().is_none());
    }

    // Job block-header matching should accept the referenced header only.
    #[test]
    fn job_contract_matches_only_its_input_block_header() {
        let owner = Owner::new_sig();
        let block_header = blk_hdr();
        let contract = JobContract::new(
            workptr(&owner, block_header.id()),
            block_header.clone(),
        );
        let other_header = BlockHeader::new(&block_header, &BlockBody::genesis());

        assert!(contract.is_same_blk_hdr(&block_header));
        assert!(!contract.is_same_blk_hdr(&other_header));
    }

    // Adding a valid result should update the result count and first-result accessor.
    #[test]
    fn adding_result_updates_job_contract() -> AgentResult<()> {
        let input_owner = Owner::new_sig();
        let result_owner = Owner::new_sig();
        let block_header = blk_hdr();
        let mut contract = JobContract::new(
            workptr(&input_owner, block_header.id()),
            block_header.clone(),
        );
        let result = resultptr(
            contract.work_id(),
            &result_owner,
            block_header.vdf_diff(),
        );
        let result_id = result.id();

        assert_eq!(contract.add_result(result)?, 1);
        assert_eq!(contract.wr_len(), 1);
        assert_eq!(contract.first_result().unwrap().id(), result_id);
        assert_eq!(contract.new_gdc(), WU::default());
        Ok(())
    }

    // Results for another work pointer must be rejected by the job contract.
    #[test]
    fn job_contract_rejects_wrong_work_pointer_reference() {
        let input_owner = Owner::new_sig();
        let result_owner = Owner::new_sig();
        let block_header = blk_hdr();
        let mut contract = JobContract::new(
            workptr(&input_owner, block_header.id()),
            block_header.clone(),
        );
        let result = resultptr(
            [99u8; 32],
            &result_owner,
            block_header.vdf_diff(),
        );

        let err = contract.add_result(result).unwrap_err();
        assert_eq!(err, "Error: Invalid WorkPtr Reference");
    }
}