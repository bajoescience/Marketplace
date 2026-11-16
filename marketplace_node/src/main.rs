use std::vec;

use marketplace_core::Core;
use marketplace_helpers::{functions, objects::{AgentResult, IdHash, WU, WorkSize}};
use marketplace_ledger::{Blockchain, DefaultBalances, State};
use marketplace_p2p::Message;
use marketplace_primitives::{par_exec::{ExecuteTime, TxOptRef, WorkPtr}, sync_final::BlockHeader};
use marketplace_wallet::Owner;

#[tokio::main]
async fn main() -> AgentResult<()> {
    // Utxo state
    let state= State::<DefaultBalances>::new();

    // Blockchain
    let chain = Blockchain::init("ledger", state).unwrap();

    // Node identity used to vote on blocks
    let owner = Owner::new_sig();

    let core = Core::new(chain, owner.clone());

    // Add Workptr message
    let msg = new_job(&owner);

    core.handle_message(&msg).await
}

fn new_job(owner: &Owner) -> Message<WorkPtr> {
    let work_addr = functions::dum_bytes();
    let work_size = WorkSize::build(WU::GDC()).unwrap();
    let ipts = vec![TxOptRef::new([255u8; 32], 0)];
    let remainder = Owner::new_sig().as_lock();
    let blk_hdr = BlockHeader::genesis().id();

    let workptr = WorkPtr::new(
        work_addr, 
        work_size, 
        ipts, remainder, blk_hdr, 
        ExecuteTime::NOW
    );

    // Sign workptr using owner
    let key = owner.sign(&workptr.id());


    let mut work_msg = Message::new(workptr);
    work_msg.add_auth(owner.as_lock(), key);
    work_msg
}
