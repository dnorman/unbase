
mod common_structs;
mod memo;
mod slabref;
mod memoref;
mod counter;
mod handle;
pub mod storage;

pub mod prelude {
    pub use slab::SlabId; // Intentionally omitting trait Slab and MemorySlab from here
    pub use slab::LocalSlabHandle;
    pub use slab::common_structs::*;
    pub use slab::memoref::{MemoRef,MemoRefInner,MemoRefPtr};
    pub use slab::memo::{MemoId,Memo,MemoInner,MemoBody};
    pub use slab::memoref::serde as memoref_serde;
    pub use slab::memo::serde as memo_serde;
    pub use slab::{SlabAnticipatedLifetime,SlabPresence};
}

/// Slab is the storage engine
pub trait Slab {
    fn get_handle (&self)    -> LocalSlabHandle;
    //fn get_ref (&self)       -> self::SlabRef;
    fn get_net (&self)       -> network::Network;
    fn create_context(&self) -> context::Context;
}


use {context, network};
use network::{Network,Transmitter,TransportAddress};
use self::common_structs::*;

pub type SlabId = u32;

/// Handle to a slab which is resident within our process
#[derive(Clone)]
pub struct LocalSlabHandle {
    pub id: SlabId,
    pub tx: LocalSlabSender,
    //pub my_ref: SlabRef,
}

/// SlabPresence is the latest info on how to reach a slab, and what sort of approximate behavior to expect from it
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlabPresence {
    pub slab_id: SlabId,
    pub address: TransportAddress,
    pub lifetime: SlabAnticipatedLifetime,
}

impl PartialEq for SlabPresence {
    fn eq(&self, other: &SlabPresence) -> bool {
        // When comparing equality, we can skip the anticipated lifetime
        self.slab_id == other.slab_id && self.address == other.address
    }
}

impl SlabPresence {
    pub fn get_handle(&self, net: &Network) -> SlabHandle {

        
        SlabHandle{
            slab_id: self.slab_id,
            tx: net.get_transmitter(address)
        }
    }
}

/// Handle for communicating with a slab that might be local OR remote
struct SlabHandle {
    pub slab_id: SlabId,
    //presence: Vec<SlabPresence>,
    tx:       Transmitter,
    return_address: TransportAddress,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SlabAnticipatedLifetime {
    Ephmeral,
    Session,
    Long,
    VeryLong,
    Unknown,
}