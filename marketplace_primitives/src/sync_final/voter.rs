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

#[cfg(test)]
mod tests {
    use marketplace_wallet::Owner;

    use crate::sync_final::{Block, Proposer, voter::Voter};

    // The Voter trait should expose the proposer identity and VRF result.
    #[test]
    fn proposer_implements_voter_accessors() {
        let owner = Owner::new_sig();
        let proposer = Proposer::new(&owner, Block::genesis());

        assert_eq!(proposer.owner(), owner.as_lock());
        assert!(proposer.get_vrf_opt().is_ok());
    }
}