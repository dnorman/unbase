
mod common_structs;
mod memo;
mod memoref;
mod counter;
mod localhandle;
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


use std::sync::Arc;

use {context, network};
use network::{Network,Transmitter,TransportAddress};
use self::common_structs::*;
use self::counter::SlabCounter;

/// Storable
pub type SlabId = u32;

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
    pub id: SlabId,
    pub tx: LocalSlabRequester,
    pub counter: Arc<SlabCounter>,
    //pub my_ref: SlabRef,
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