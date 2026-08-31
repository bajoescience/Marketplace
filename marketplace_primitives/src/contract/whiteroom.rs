use std::{collections::{HashMap}, format};

use arrayvec::ArrayVec;
use borsh::{BorshDeserialize, BorshSerialize};
use marketplace_helpers::{functions, objects::{AgentResult, ID, WHITEROOM_MAX}};

// Consensus trait to prove what it means 
// for a whiteroom to agree
pub trait WRVote: PartialEq {
    fn as_vote(&self) -> ID;
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Whiteroom<T: WRVote> {
    list: ArrayVec<T, WHITEROOM_MAX>,
    votes: HashMap<ID, usize>,
}

impl<T> Whiteroom<T>
where
    T: WRVote 
{
    pub fn new() -> Whiteroom<T> {
        Self {
            list: ArrayVec::new(),
            votes: HashMap::new(),
        }
    }
}

// Getter methods
impl<T> Whiteroom<T>
where
    T: WRVote 
{
    // Whiteroom member count
    pub fn len(&self) -> usize {
        self.list.len()
    }

    // Whiteroom members
    pub fn members(&self) -> impl Iterator<Item = &T> {
        self.list.iter()
    }

    // Gwt winning members
    pub fn winners(&self) -> impl Iterator<Item = &T> {
        self.list
            .iter()
            .filter(
                |member| {
                    if let Some(winning_vote) = self.winning_vote() {
                        member.as_vote() == *winning_vote.0
                    } else {
                        false
                    }   
                }
            )
    }

    // Checks if current members have reached consensus
    pub fn is_consensus(&self) -> bool {
        // Check if winning vote has greater count than threshold
        if let Some(winner) = self.winning_vote() {
            if *winner.1 >= functions::bft_thresh(WHITEROOM_MAX) {
                return true;
            }
        }

        false
    }

    // Check if consensus is still possible
    pub fn can_consensus(&self) -> bool {
        // Whiteroom can only reach consensus when less than 2
        // results reach effective threshold
        let effective_thresh = WHITEROOM_MAX - functions::bft_thresh(WHITEROOM_MAX);

        let count = self.votes
            .iter()
            .filter(
                |&(_, vote)|
                 *vote > effective_thresh
            )
            .count();

        if count < 2 {
            true
        } else {
            false
        }
    }

    // Get winning votes
    pub fn winning_vote(&self) -> Option<(&ID, &usize)> {
        self.votes.iter().max_by_key(|&(_, vote)| *vote)
    }
}

// Setter methods
impl<T> Whiteroom<T>
where
    T: WRVote 
{

    // Add a new whiteroom member
    pub fn add_member(&mut self, member: T) -> AgentResult<usize> {
        if self.list.is_full() {
            return Err(format!(
                "Error: Whiteroom is already full"
            ))
        }

        // Increase whiteroom member vote's count
        *self.votes.entry(member.as_vote()).or_insert(0) += 1;

        // Ensure no duplicates
        if self.list.contains(&member) {
            return Err(format!(
                "Error: Duplicate Whiterooom Member"
            ))
        }

        self.list.push(member);
        Ok(self.len())
    }
}

#[cfg(test)]
mod tests {
    use std::assert_eq;

use super::*;

    #[derive(PartialEq)]
    // (number, vote)
    pub struct TestMember(u8, ID);

    impl WRVote for TestMember {
        fn as_vote(&self) -> ID {
            self.1
        }
    }

    // Add a member to a whiteroom
    #[test]
    fn add_member_whiteroom() -> AgentResult<()> {
        let member = TestMember(
            0,
            functions::dum_bytes()
        );

        let mut whiteroom = Whiteroom::new();

        whiteroom.add_member(member)?;

        assert_eq!(whiteroom.len(), 1);
        Ok(())
    }

    // Add duplicate members should fail
    #[test]
    fn add_dup_member_whiteroom() -> AgentResult<()> {
        // Duplicate members
        let member0 = TestMember(
            0,
            functions::dum_bytes()
        );

        let member = TestMember(
            0,
            functions::dum_bytes()
        );

        let mut whiteroom = Whiteroom::new();

        whiteroom.add_member(member0)?;

        let err = whiteroom.add_member(member).unwrap_err();

        assert_eq!(err, "Error: Duplicate Whiterooom Member");
        Ok(())
    }

    // Test whiteroom consensus should work
    #[test]
    fn is_consensus_whiteroom() -> AgentResult<()> {
        let mut wr = Whiteroom::new();

        
        // Create 2 members
        // This is less than whiteroom's BFT threshold
        // can't reach consensus
        for i in 0u8..2 {
            let member = TestMember(i, [0; 32]);
            wr.add_member(member)?;
        }

        assert!(!wr.is_consensus());

        // Add more members
        for i in 2u8..3 {
            let member = TestMember(i, [0; 32]);
            wr.add_member(member)?;
        }

        assert!(wr.is_consensus());

        Ok(())
    }

    // Test whiteroom non-consensus should fail
    #[test]
    fn is_non_consensus_whiteroom() -> AgentResult<()> {
        let mut wr = Whiteroom::new();

        // Testing 3(1) + 1 members
        // We create 2 agreeing members
        for i in 0u8..2 {
            let member = TestMember(i, [0; 32]);
            wr.add_member(member)?;
        }

        // Disagreeing members 2
        for i in 2u8..4 {
            let member = TestMember(i, [5; 32]);
            wr.add_member(member)?;
        }

        // Should not reach consensus
        assert_eq!(*wr.winning_vote().unwrap().1, 2);
        assert!(!wr.is_consensus());
        assert!(!wr.can_consensus());
        Ok(())
    }
}