mod common_structs;
pub (crate) mod memo;
pub (crate) mod memoref;
mod counter;
mod localhandle;
mod dispatcher;
mod store;

pub use self::store::memory::MemoryStore as Memory;

use util::workeragent::WorkerAgent;
use self::dispatcher::Dispatcher;
use context::Context;

use std::fmt;

pub mod prelude {
    // Intentionally omitting trait Slab and MemorySlab from here
    // Intentionally omitting SlabId from here ( because I don't want anyone outside the slab module to use it)
    pub use slab::SlabRef;
    pub use slab::LocalSlabHandle;
    pub use slab::common_structs::*;
    pub use slab::memoref::MemoRef;
    pub use slab::memo::{MemoId,Memo,MemoBody};
    pub use slab::{SlabAnticipatedLifetime,SlabPresence};
    pub use slab::memo::peerstate::{MemoPeerSet,MemoPeerState,MemoPeerStatus};
}


pub struct Slab<S=Memory> {
    slab_id: SlabId,
    store: WorkerAgent<SlabStoreWorker<S>>,
    counter: Rc<SlabCounter>,
    dispatcher: WorkerAgent<Dispatcher<S>>,
    net: Network
}

impl <S> Slab<S> {
    pub fn new(net: &Network) -> Self<S> {
        let slab_id = net.generate_slab_id();

        let counter = Rc::new(SlabCounter::new());
        let store: WorkerAgent<StoreWorker<S>> = WorkerAgent::new(S::new(
            slab_id,
            net.clone(),
            counter.clone(),
            dispatcher_tx
        ));

        let dispatcher: WorkerAgent<Dispatcher<S>> = Dispatcher::new(net.clone(), store.clone() );

        let me = Slab::<S> {
            slab_id,
            store,
            counter,
            dispatcher,
            net: net.clone(),
        };

        net.register_local_slab(me.get_handle());
        net.conditionally_generate_root_index_seed(&me.get_handle());

        me
    }
    fn slab_id (&self) -> SlabId{
        self.slab_id.clone()
    }
    fn get_handle (&self) -> LocalSlabHandle {
        LocalSlabHandle::new( self.get_slabref(), self.counter.clone(), self.core.clone() )
    }
    fn get_slabref (&self) -> SlabRef {
        SlabRef{
            owning_slab_id: self.slab_id,
            slab_id: self.slab_id
        }
    }
    fn get_net (&self) -> Network {
        self.net.clone()
    }
    fn create_context (&self) -> Context {
        Context::new(self)
    }
}

impl <S> Drop for Slab<S> {
    fn drop(&mut self) {

        //println!("# SlabInner({}).drop", self.id);
        // self.memoref_dispatch_tx_channel.take();
        // if let Some(t) = self.memoref_dispatch_thread.write().unwrap().take() {
        //     t.join().expect("join memoref_dispatch_thread");
        // }
        self.net.deregister_local_slab(self.get_slabref());
        // TODO: Drop all observers? Or perhaps observers should drop the slab (weak ref directionality)
    }
}

impl <S> fmt::Debug for Slab<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Slab")
            .field("slab_id", &self.slab_id)
            .finish()
    }
}

use std::rc::Rc;
use std::cell::RefCell;

use {context, network};
use network::{Network,Transmitter,TransportAddress};
use self::counter::SlabCounter;
use self::store::StoreHandle;

/// The actual identifier for a slab Storable
pub type SlabId = u32;

/// `SlabRef` is how we refer to a slab
/// The intent is to possibly convert this to an arc, or some other pointer in the future, rather than copying the full identifier all over the place
/// One should only be able to get a `SlabRef` from a Slab, because it's in charge of storing Slab IDs.
#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialOrd, Ord)]
pub struct SlabRef {
    owning_slab_id: SlabId,
    pub slab_id: SlabId
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub enum SlabAnticipatedLifetime {
    Ephmeral,
    Session,
    Long,
    VeryLong,
    Unknown,
}

/// `SlabPresence` is the latest info on how to reach a slab, and what sort of approximate behavior to expect from it.
/// (Storable)
#[derive(Debug, Clone, Serialize, Deserialize, Eq, Ord, PartialOrd)]
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
    pub store: StoreHandle,
    pub counter: Rc<SlabCounter>,
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
    pub fn hack ( slab_id: SlabId, owning_slab_id: SlabId ) -> Self {
        SlabRef{ slab_id, owning_slab_id }
    }
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
    pub fn return_presence (&self) -> SlabPresence {
        // Get the address that the remote slab would recogize
        unimplemented!()
        // SlabPresence {
        //     slab_id: self.slab_id,
        //     addresses: origin_slabref.get_return_addresses(),
        //     lifetime: SlabAnticipatedLifetime::Unknown
        // }
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

impl SlabPresence {
    pub fn get_transmitter (&self, _net: &Network) -> Option<Transmitter> {

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