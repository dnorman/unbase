pub mod serde;
use crate::slab::*;
use std::fmt;

#[derive(Clone)]
pub struct Packet {
    pub to_slab_id: SlabId,
    pub from_slab_id: SlabId,
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
