use std::{collections::HashMap, format};

use marketplace_helpers::objects::{AgentResult, ID, IdHash};
use marketplace_wallet::{Key, Lock};

// Message has different variants for all types
// This includes the following
// Job: To create a new job on network, 
// Result: To add job results to an existing job in network,
// Block: To add a new Block to a job in the network,
// Confirm: To confirm the chosen block.
#[derive(Debug)]
pub struct Message<T: IdHash> {
    pub payload: T,
    pub auth: HashMap<ID, Key>,
}

impl<T: IdHash> Message<T> {
    pub fn authenticate(&self, lock: Lock) -> AgentResult<()> {
        let Message { payload, auth } = self;

        let Some(key) = auth.get(&lock.id()) else {
            return Err(format!(
                "Error: Key does not exist"
            ));
        };

        lock.unlock(key, &payload.id())
    }
}