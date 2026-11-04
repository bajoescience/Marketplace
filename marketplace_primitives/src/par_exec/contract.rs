pub mod job;
pub mod tx;

pub use self::job::JobContract;
pub use self::tx::TxContract;

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    helpers::objects::{AgentResult, ID, WU},
    Verify,
};

// Contract type for a job
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum Contract {
    JOB(JobContract),
    TX(TxContract)
}

impl Contract {
    // Genesis Contract
    pub fn genesis() -> Self {
        Self::TX(TxContract::genesis())
    }

    // Calculate total new coins in a tx
    pub fn new_coins(&self) -> WU {
        if let Self::JOB(jobctr) = self {
            jobctr.new_gdc()
        } else {
            WU::default()
        }
    }

    pub fn id(&self) -> ID {
        match self {
            Self::JOB(jobctr) => jobctr.id(),
            Self::TX(txctr) => txctr.id()
        }
    }
}

impl PartialEq for Contract {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for Contract {
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
    use std::thread;

    use crate::{
        helpers::objects::{AgentResult, CONTRACT_WHITEROOM_SIZE, IdHash, WU},
        par_exec::{
            Bill, Tx, TxIdentifier, TxOpt, TxOptRef,
            contract::job::tests::{resultptr, workptr},
        },
        sync_final::BlockHeader,
        wallet::Owner,
        Verify,
    };

    use super::{Contract, JobContract, TxContract};

    // Test Job contract
    pub fn jobctr() -> AgentResult<JobContract>  {
        let ipt_owner = Owner::new_sig();

        // Block header
        let blk_hdr = BlockHeader::genesis();

        // Input
        let workptr = workptr(&ipt_owner, blk_hdr.id());
        let mut jobctr = JobContract::new(workptr, blk_hdr.clone());
        
        // ResultPtr Initializers
        let work_id = jobctr.input().id();

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
        let id = jobctr.id();

        let ctr = Contract::JOB(jobctr);

        assert_eq!(ctr.id(), id);
        Ok(())
    }

    // Test Tx contracts
    pub fn txctr() -> AgentResult<TxContract> {
        let owner = Owner::new_sig();

        let ipts = vec![
            TxOptRef::new([255; 32], 0)
        ];

        let opts = Some(vec![
            TxOpt::new(owner.as_lock(), WU::try_from(20000).unwrap())
        ]);
        
        let tx = Tx::new(0, TxIdentifier::COIN, Some(ipts), opts);

        let bill = Bill::new(vec![tx]);
        let key = owner.sign(&bill.id());

        let mut txctr = TxContract::new(bill);

        txctr.add_auth(owner.as_lock(), key);

        Ok(txctr)
    }

    // Verify Tx contract
    #[test]
    fn verify_txctr() -> AgentResult<()> {
        let ctr = Contract::TX(txctr()?);
        ctr.verify()
    }

    // Genesis should create a transaction contract containing the initial ledgers.
    #[test]
    fn genesis_contract_contains_initial_transactions() {
        let contract = Contract::genesis();

        let tx_contract = match contract {
            Contract::TX(tx_contract) => tx_contract,
            Contract::JOB(_) => panic!("Genesis contract must be a transaction contract"),
        };

        assert_eq!(tx_contract.get_txs().len(), 2);
        assert!(tx_contract.get_txs()[0].is_coin());
        assert!(!tx_contract.get_txs()[1].is_coin());
    }

    // Contract identifiers should delegate to the wrapped contract type.
    #[test]
    fn contract_identifier_delegates_to_inner_contract() -> AgentResult<()> {
        let tx_contract = txctr()?;
        let expected_id = tx_contract.id();
        let contract = Contract::TX(tx_contract);

        assert_eq!(contract.id(), expected_id);
        Ok(())
    }

    // Transaction contracts do not create newly issued goldcoin.
    #[test]
    fn transaction_contract_has_no_new_coins() -> AgentResult<()> {
        let contract = Contract::TX(txctr()?);

        assert_eq!(contract.new_coins(), WU::default());
        Ok(())
    }
}