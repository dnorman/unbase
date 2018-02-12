mod core;

//use futures::prelude::*;
use std::sync::Arc;

use self::core::MemoryCore;
use network::{Network};
use slab::Slab;
use slab::storage;
use slab::prelude::*;
use slab::counter::SlabCounter;
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
use std::thread;
// use std::time;
// use futures::{Future, Sink};

pub struct Memory{
    slabref: SlabRef,
    core_thread: storage::CoreThread,
    counter: Arc<SlabCounter>,
    my_handle: LocalSlabHandle,
    net: Network
}

impl Slab for Memory {
    fn get_handle (&self) -> LocalSlabHandle {
        self.my_handle.clone()
    }
    fn get_slabref (&self) -> SlabRef {
         self.slabref.clone()
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

        let core = MemoryCore::new(
            slab_id,
            net.clone(),
            counter.clone()
        );

        // TODO: Under single threaded mode this should be Rc<StorageCore>
        let core_thread = storage::CoreThread::new(Box::new(core));

        let my_handle = LocalSlabHandle::new( slab_id, counter.clone(), core_thread.requester() );

        let me = Memory{
            slab_id,
            core_thread,
            counter,
            my_handle,
            net: net.clone(),
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
        self.net.deregister_local_slab(self.slab_id);
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