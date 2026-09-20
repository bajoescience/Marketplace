pub mod job;
pub mod tx;

use crate::Verify;

pub use self::job::JobContract;
pub use self::tx::TxContract;

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::objects::{AgentResult, ID, IdHash, WU};

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

    pub fn ctr_id(&self) -> ID {
        match self {
            Self::JOB(jobctr) => jobctr.work_id(),
            Self::TX(txctr) => txctr.id()
        }
    }
}

impl Verify for Contract {
    fn verify(&self) -> AgentResult<()> {
        match self {
            Self::JOB(jobctr) => jobctr.verify(),
            Self::TX(txctr) => txctr.verify()
        }
    }
}

// Tests here
#[cfg(test)]
pub mod tests {
    use std::{assert_eq, thread};

use marketplace_helpers::objects::{AgentResult, WHITEROOM_SIZE};
    use marketplace_wallet::Owner;
    use crate::{BlockHeader, Tx, TxIO, TxIdentifier, par_exec::contract::job::tests::{resultptr, workptr}};
    use super::*;

    // Test Job contract
    pub fn jobctr() -> AgentResult<JobContract>  {
        let ipt_owner = Owner::new_sig();

        // Block header
        let blk_hdr = BlockHeader::genesis();

        // Input
        let workptr = workptr(&ipt_owner, blk_hdr.id());
        let mut jobctr = JobContract::new(workptr, blk_hdr);
        
        // ResultPtr Initializers
        let work_id = jobctr.input().id();

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
            jobctr.add_result(result_ptr)?;
        }

        Ok(jobctr)
    }

    // Verify Job contract
    #[test]
    fn verify_jobctr() -> AgentResult<()> {
        let ctr = Contract::JOB(jobctr()?);

        ctr.verify()
    }

    // Verify job contract id
    #[test]
    fn verify_jobctr_id() -> AgentResult<()> {
        let jobctr = jobctr()?;
        let id = jobctr.work_id();

        let ctr = Contract::JOB(jobctr);

        assert_eq!(ctr.ctr_id(), id);
        Ok(())
    }

    // Test Tx contracts
    pub fn txctr() -> AgentResult<TxContract> {
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

        Ok(txctr)
    }

    // Verify Tx contract
    #[test]
    fn verify_txctr() -> AgentResult<()> {
        let ctr = Contract::TX(txctr()?);
        ctr.verify()
    }
}