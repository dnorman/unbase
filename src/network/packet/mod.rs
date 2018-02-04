pub mod serde;
use slab::prelude::*;
use std::fmt;

#[derive(Clone)]
pub struct Packet {
    pub to_slabref: SlabRef,
    pub from_slabref: SlabRef,
    pub memo: Memo,
    pub peerlist: MemoPeerList,
}

impl fmt::Debug for Packet {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Packet")
            .field("from_slab_id", &self.from_slab_id)
            .field("to_slab_id", &self.to_slab_id)
            .field("memo", &self.memo)
            .field("peerlist", &self.peerlist)
            .finish()
    }
}
