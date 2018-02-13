mod core;

use std::sync::Arc;

use self::core::MemoryCore;
use network::{Network};
use slab::{self, dispatcher::*, storage::{self, *}, prelude::*, Slab, counter::SlabCounter};
use context::Context;


// use subject::{SubjectId,SubjectType};
// use memorefhead::*;
// use error::*;

// use std::ops::Deref;
// use std::sync::{Arc,Weak,RwLock,Mutex};
// use std::sync::mpsc;
// use std::sync::mpsc::channel;
// use std::collections::HashMap;
// use std::collections::hash_map::Entry;
use std::fmt;
// use std::time;
// use futures::{Future, Sink};

pub struct Memory{
    slab_id: slab::SlabId,
    core_thread: storage::CoreThread,
    storage_requester: StorageRequester,
    counter: Arc<SlabCounter>,
    net: Network
}

impl Slab for Memory {
    fn get_handle (&self) -> LocalSlabHandle {
        LocalSlabHandle::new( self.get_slabref(), self.counter.clone(), self.storage_requester.clone() )
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

impl Memory {
    pub fn new(net: &Network) -> Self {
        let slab_id = net.generate_slab_id();

        // TODO: Delete this
        // let my_ref = SlabRef::new(
        //     slab_id,
        //     slab_id, // I own my own ref to me, obviously
        //     Transmitter::new_blackhole(slab_id)
        // );

        let counter = Arc::new(SlabCounter::new());
        let (storage_requester, requester_rx) = StorageRequester::new();
        let dispatcher = Dispatcher::new( storage_requester.clone(), counter.clone() );

        let core = MemoryCore::new(
            slab_id,
            net.clone(),
            counter.clone(),
            dispatcher.tx.clone()
        );

        // TODO: Under single threaded mode this should be Rc<StorageCore>
        let core_thread = storage::CoreThread::new(Box::new(core), requester_rx);

        let me = Memory{
            slab_id,
            core_thread,
            counter,
            net: net.clone(),
            storage_requester,
        };

        net.register_local_slab(me.get_handle());
        net.conditionally_generate_root_index_seed(&me.get_handle());

        me
    }
}

impl Drop for Memory {
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

impl fmt::Debug for Memory {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Slab")
            .field("slab_id", &self.slab_id)
            .finish()
    }
}