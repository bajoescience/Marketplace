use marketplace_primitives::sync_final::{BlockBody, SubBlock};

pub struct Mempool {
    block_body: Option<BlockBody>
}

impl Mempool {
    pub fn new() -> Self {
        Self {
            block_body: Some(BlockBody::new())
        }
    }
}

// Setter methods
impl Mempool {
    pub fn add_sub_block(&mut self, sub_block: SubBlock) {
        if let Some(block_body) = &mut self.block_body {
            block_body.add(sub_block);
        }
    }
}