mod state;
mod mempool;

use std::{collections::{HashMap, VecDeque}, format, fs, todo, vec};

use marketplace_helpers::{functions, objects::{AgentResult, ID, IdHash, WU}};

use marketplace_primitives::{Block, BlockHeader, Contract, Verify};

pub use state::{State};
pub use mempool::Mempool;

// Blockchain
// Store list of block headers
// The block and headers are kept in a file
// while only the headers are kept in memory
pub struct Blockchain<T: State> {
    // Store BlockHeader ID as key
    headers: HashMap<ID, BlockHeader>,

    // Latest Block headers
    latest: VecDeque<ID>,

    // Total amount of gdc in existence
    total: WU,

    // Store state that can be used as a checkpoint
    state: T,

    blockfile: Vec<Block>,
}

impl<T: State> Blockchain<T> {
    /// Initialize a new Blockchain
    /// This is used in the initialization 
    /// of the Marketplace State
    pub fn new(state: T) -> Self {
        // Add genesis header
        let headers = HashMap::new();
        let latest = VecDeque::with_capacity(2);

        let genesis_blk = Block::genesis();
        let genesis_hdr = BlockHeader::genesis(); 

        let mut chain = Self { 
            headers,
            latest,
            total: genesis_hdr.new_gdc(),
            state,
            blockfile: vec![Block::genesis()],
        };

        // Update latest and add headers to chain
        chain.append_to_chain(genesis_hdr);

        chain
    }

    // Validate a contract
    fn validate_contract(&self, ctr: &Contract) -> AgentResult<()> {
        match ctr {
            Contract::TX(txctr) => txctr.verify(),
            Contract::JOB(jobctr) => {
                // Get block header referenced by contract
                let Some(_) = self.find_hdr(&jobctr.get_input_blk_hdr().id()) else {
                return Err(format!(
                    "Error: Block Header {} does not exist in Blockchain",
                        functions::from_bytes(&jobctr.get_input_blk_hdr().id()) 
                    ))
                };

                // Verify Job contract
                jobctr.verify()
            },
        }
    }

    // Validate a block
    pub fn validate_block(&self, block: &Block) -> AgentResult<()> {
        for ctr in block.body() {
            self.validate_contract(ctr)?;
        }

        Ok(())
    }
}

// Getter methods
impl<T: State> Blockchain<T> {
    // Find header using ID
    pub fn find_hdr(&self, id: &ID) -> Option<&BlockHeader> {
        self.headers.get(id)   
    }

    // latest block_headers
    pub fn latest_hdrs(&self) -> &VecDeque<ID> {
        &self.latest
    } 

    // Latest header
    pub fn latest_hdr_id(&self) -> &ID {
        self.latest
            .back()
            .expect(
                "Error: Blockchain has no Blocks, genesis block is missing!"
            )
    }

    pub fn latest_hdr(&self) -> &BlockHeader {
        self.headers
            .get(self.latest_hdr_id())
            .unwrap()
    }

    // Get average from a blockheader using blockheader ID
    pub fn get_average_using(&self, hdr_id: &ID) -> AgentResult<WU> {
        let Some(hdr) = self.find_hdr(hdr_id) else {
            return Err(format!(
                "Error: Header of id {} could not be found",
                functions::from_bytes(hdr_id)
            ))
        };

        Ok(hdr.average())
    }

    // Get newest average
    pub fn current_average(&self) -> WU {
        let hdr = self.latest_hdr();
        hdr.average()
    }

    // Create a new Block header using Block
    pub fn header_from(&self, block: &Block) -> BlockHeader {
        BlockHeader::new(
            block, 
            self.latest_hdr()
        )
    }

    // Total goldcoin in existence
    pub fn total_gdc(&self) -> WU {
        self.total
    }

    // Update state given to it
    pub fn latest_state(&self) -> impl State {
        self.state.clone()
    }
}

// Setter methods
impl<T: State> Blockchain<T> {
    // Add to file
    fn add_to_file(&self, block: Block) -> AgentResult<()> {
        let bytes = block.serialize();

        let Ok(_) = fs::write("blockfile.dat", bytes) else {
            return Err(format!(
                "Error: Error adding Block to Blockfile"
            ));
        };

        Ok(())
    }

    // Read from file
    fn read_from_file(&self) -> AgentResult<()> {
        todo!()
    }

    // Append header to chain
    fn append_to_chain(&mut self, hdr: BlockHeader) {

        // Update latest header
        if self.latest.len() == 2 {
            self.latest.pop_front();
        }

        self.latest.push_back(hdr.id());

        // Add header to chain
        self.headers.insert(hdr.id(), hdr);
    }

    // Update State using tx in a block
    // If error, revert registered changes
    fn update_state_with_ctr(&mut self, ctr: &Contract) -> AgentResult<()> {
        match ctr {
                Contract::JOB(jobctr) => {
                    let tx = jobctr.get_tx();

                    match self.state.try_update(&tx) {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            self.state.revert(&tx)
                                .expect("Error reverting Tx");

                            Err(e)
                        }
                    }?;
                },
                Contract::TX(txctr) => {
                    let mut txs = Vec::new();

                    for tx in txctr.get_tx() {
                        match self.state.try_update(tx) {
                            Ok(_) => txs.push(tx),
                            Err(e) => {
                                // Roll back all txs
                                for tx in txs {
                                    self.state.revert(&tx)
                                        .expect("Error reverting Tx");
                                }

                                return Err(e);
                            }
                        };
                    }
                },
            }

        Ok(())
    }

    // Reverse state with contract
    // Any errors encountered during reverting is
    // an anomaly, and the program should panic
    fn revert_state_with_ctr(&mut self, ctr: &Contract) {
        match ctr {
            Contract::JOB(jobctr) => {
                self.state.revert(&jobctr.get_tx())
                    .expect("Error reverting Tx");
            },
            Contract::TX(txctr) => {
                for tx in txctr.get_tx() {
                    self.state.revert(tx)
                        .expect("Error reverting Tx");
                }
            }
        }
    }

    /// Add a new block to the Blockchain
    /// A new BlockHeader instance is created and stored
    /// in memory by the Blockchain
    /// 
    /// This method takes a Block from the mempool and
    /// adds it to the chain
    pub fn add_block(&mut self, block: Block) -> AgentResult<()> {

        // Confirm and validate block
        self.validate_block(&block)?;

        // Generate block header
        let header = self.header_from(&block);

        // Add block to state
        // if error encounterd in contract
        // revert changes and discard block
        let mut ctrs = Vec::new();

        for ctr in block.body() {
            match self.update_state_with_ctr(&ctr) {
                Ok(_) => ctrs.push(ctr),
                Err(e) => {
                    for ctr in ctrs {
                        self.revert_state_with_ctr(&ctr)
                    }
                    return Err(e)
                }
            };
        }

        // TODO: Store block in file
        self.append_to_chain(header);
        self.blockfile.push(block);

        Ok(())
    }

    // TODO: Add received finalized block to mempool
}


#[cfg(test)]
mod tests {
    use crate::state::DefaultState;

use super::*;
    use std::{assert_eq, vec};
    use marketplace_primitives::{Tx, TxContract, TxIO, TxIdentifier};
use marketplace_wallet::Owner;

    // Test Transactions where owner sends money to another and 
    // the rest to himself
    fn ctr(owner: &Owner, owner1: &Owner) -> Contract {

        let ipt = Some(vec![TxIO::new(
            owner.as_lock(), 
            WU::GDC(),
        )]);

        let opt = Some(vec![
            TxIO::new(
                owner1.as_lock(), 
                WU::try_from(30000).unwrap(),
            ), 
            TxIO::new(
                owner.as_lock(), 
                WU::GDC() - WU::try_from(30000).unwrap(),
            )
        ]);

        let txs = vec![
            Tx::new(
                0, 
                TxIdentifier::COIN, 
                ipt.clone(), 
                opt.clone()
            )
        ];

        let mut txctr = TxContract::new(txs);
        let key = owner.sign(&txctr.id());

        txctr.add_auth(
            owner.as_lock(), 
            key, 
            false
        ).unwrap();

        Contract::TX(txctr)
        
    }

    // Add an empty block to the blockchain
    #[test]
    fn add_empty_block_to_blockchain() -> AgentResult<()> {
        let state = DefaultState::new();

        let mut chain = Blockchain::new(state);

        let block = Block::new(vec![]);

        chain.add_block(block)?;

        assert_eq!(chain.headers.len(), 2);
        Ok(())
    }

    // Update a Default state passed to the blockchain
    #[test]
    fn update_state_with_blockchain() -> AgentResult<()> {
        let mut state = DefaultState::new();

        // Test owners
        let owner = Owner::new_sig();
        let owner1 = Owner::new_sig();

        // Give owner 1 gdc
        let opt = vec![
            TxIO::new(
                owner.as_lock(), 
                WU::GDC()
            )
        ];

        state.try_update(
            &Tx::new(0, TxIdentifier::COIN, None, Some(opt))
        )?;

        // Add state to blockchain
        let mut chain = Blockchain::new(state);

        // Block with tx where owner send some gdc to another
        let block = Block::new(vec![ctr(&owner, &owner1)]);

        chain.add_block(block)?;

        // Confirm owner's balances
        assert_eq!(
            chain.latest_state().get_gdc_balance(owner.as_lock()),
            WU::GDC() - WU::try_from(30000).unwrap()
        );

        assert_eq!(
            chain.latest_state().get_gdc_balance(owner1.as_lock()),
            WU::try_from(30000).unwrap()
        );

        Ok(())
    }

    // TODO: Test adding a normal block to the chain

    // TODO: Test adding a faulty block to the chain
}