mod serde;
mod common_structs;
mod memo;
mod memoref;
mod counter;
mod localhandle;
pub mod storage;


pub mod prelude {
    // Intentionally omitting trait Slab and MemorySlab from here
    // Intentionally omitting SlabId from here ( because I don't want anyone outside the slab module to use it)
    pub use slab::SlabRef;
    pub use slab::LocalSlabHandle;
    pub use slab::common_structs::*;
    pub use slab::memoref::{MemoRef,MemoRefInner,MemoRefPtr};
    pub use slab::memo::{MemoId,Memo,MemoBody};
    pub use slab::memoref::serde as memoref_serde;
    pub use slab::memo::serde as memo_serde;
    pub use slab::{SlabAnticipatedLifetime,SlabPresence};
}

/// Slab is the storage engine
pub trait Slab {
    fn get_handle (&self)    -> LocalSlabHandle;
    fn get_slabref (&self)   -> self::SlabRef;
    fn get_net (&self)       -> network::Network;
    fn create_context(&self) -> context::Context;
}


use std::sync::Arc;

use {context, network};
use network::{Network,Transmitter,TransportAddress};
use self::common_structs::*;
use self::counter::SlabCounter;


/// The actual identifier for a slab Storable
pub type SlabId = u32;

/// SlabRef is how we refer to a slab
/// The intent is to possibly convert this to an arc, or some other pointer in the future, rather than copying the full identifier all over the place
/// One should only be able to get a SlabRef from a Slab, because it's in charge of storing Slab IDs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SlabRef {
    owning_slab_id: SlabId,
    slab_id: SlabId
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SlabAnticipatedLifetime {
    Ephmeral,
    Session,
    Long,
    VeryLong,
    Unknown,
}

/// SlabPresence is the latest info on how to reach a slab, and what sort of approximate behavior to expect from it.
/// (Storable)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlabPresence {
    pub slab_id: SlabId,
    pub address: TransportAddress,
    pub lifetime: SlabAnticipatedLifetime,
}

/// Handle to a slab which is resident within our process.
/// (Not storable)
#[derive(Clone)]
pub struct LocalSlabHandle {
    pub slabref: SlabRef,
    pub tx: LocalSlabRequester,
    pub counter: Arc<SlabCounter>,
    pub my_ref: SlabRef,
}

/// Handle for communicating with a slab that might be local OR remote
/// (Not storable)
struct SlabHandle {
    pub slab_id: SlabId,
    tx:       Transmitter,
    return_address: TransportAddress,
}

impl PartialEq for SlabPresence {
    fn eq(&self, other: &SlabPresence) -> bool {
        // When comparing equality, we can skip the anticipated lifetime
        self.slab_id == other.slab_id && self.address == other.address
    }
}

impl SlabRef {
    pub fn slab_id(&self) -> SlabId {
        self.slab_id
    }
    pub fn unknown() -> Self {
        SlabRef{ slab_id: 0 }
    }
}

impl SlabPresence {
    pub fn get_transmitter (&self, net: &Network) -> Option<Transmitter> {

        use network::TransmitterArgs;
        let args = if self.address.is_local() {
            if let Some(ref slab) = net.get_slab_handle(self.slab_id) {
                TransmitterArgs::Local(slab)
            }else{
                return None;
            }
        }else{
            TransmitterArgs::Remote( &self.slab_id, &self.address )
        };

        net.get_transmitter(&args)
    }
}