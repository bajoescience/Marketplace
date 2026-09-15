pub mod job;
pub mod tx;

pub use self::job::JobContract;
pub use self::tx::TxContract;

use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::objects::{WU};

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

// Tests here