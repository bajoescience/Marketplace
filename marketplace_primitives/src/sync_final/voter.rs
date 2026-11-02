pub mod proposer;
pub mod validator;

use marketplace_helpers::objects::{AgentResult, ID};
use marketplace_wallet::Lock;

pub use self::proposer::{Proposer, Proposal};
pub use self::validator::{Validator, Validated};

pub trait Voter {
    fn owner(&self) -> Lock;

    fn get_vrf_opt(&self) -> AgentResult<ID>;
}