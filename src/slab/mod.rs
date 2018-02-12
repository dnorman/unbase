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
    pub use slab::{MemoPeerState,MemoPeerStatus};
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
use self::counter::SlabCounter;


/// The actual identifier for a slab Storable
pub type SlabId = u32;

/// SlabRef is how we refer to a slab
/// The intent is to possibly convert this to an arc, or some other pointer in the future, rather than copying the full identifier all over the place
/// One should only be able to get a SlabRef from a Slab, because it's in charge of storing Slab IDs.
#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub addresses: Vec<TransportAddress>,
    pub lifetime: SlabAnticipatedLifetime,
}

/// Handle to a slab which is resident within our process.
/// (Not storable)
#[derive(Clone)]
pub struct LocalSlabHandle {
    pub slab_id: SlabId,
    pub slabref: SlabRef,
    // under single threaded mode, this should be Rc<StorageCore>
    pub storage: self::storage::StorageRequester,
    pub counter: Arc<SlabCounter>,
}

// /// Had to impl clone manually due to StorageInterfaceClonable
// impl Clone for LocalSlabHandle{
//     fn clone(&self) -> Self {
//         LocalSlabHandle {
//             slab_id: self.slab_id.clone(),
//             slabref: self.slabref.clone(),
//             storage: self.storage.clone(),
//             counter: self.counter.clone(),
//         }       
//     }
// }

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
        self.slab_id == other.slab_id && self.addresses == other.addresses
    }
}

impl SlabRef {
    pub fn slab_id(&self) -> SlabId {
        self.slab_id
    }
    pub fn unknown(slab: &LocalSlabHandle) -> Self {
        SlabRef{
            owning_slab_id: slab.slab_id(),
            slab_id: 0
        }
    }
    pub fn clone_for_slab(&self, to_slab: &LocalSlabHandle) -> Self {
        SlabRef{
            owning_slab_id: to_slab.slab_id(),
            slab_id: self.slab_id
        }
    }
}

impl PartialEq for SlabRef {
    fn eq(&self, other: &SlabRef) -> bool {
        // TODO - Once this is converted to use an internal Arc, first compare pointer address

        // In the case that we're comparing two slabrefs owned by different slabs, 
        // look up the slab_id itself, and use that as a fallback comparison
        self.slab_id == other.slab_id
    }
}

#[derive(Clone, Debug)]
pub struct MemoPeerState {
    pub slabref: SlabRef,
    pub status: MemoPeerStatus,
}

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub enum MemoPeerStatus {
    Resident,
    Participating,
    NonParticipating,
    Unknown,
}

impl SlabPresence {
    pub fn get_transmitter (&self, net: &Network) -> Option<Transmitter> {

        unimplemented!()
        // use network::TransmitterArgs;
        // let args = if self.address.is_local() {
        //     if let Some(ref slab) = net.get_local_slab_handle(self.slabref) {
        //         TransmitterArgs::Local(slab)
        //     }else{
        //         return None;
        //     }
        // }else{
        //     TransmitterArgs::Remote( &self.slab_id, &self.address )
        // };

        // net.get_transmitter(args)
    }
}