use std::rc::Rc;

use super::core::MemoryCore;
use network::{Network};
use slab::{self, dispatcher::*, storage::{self, *}, prelude::*, Slab, counter::SlabCounter};
use context::Context;

use std::fmt;

pub struct Memory<I> where I: StorageCoreInterface {
    slab_id: slab::SlabId,
    storage_requester: I,
    counter: Rc<SlabCounter>,
    net: Network
}

impl <I> Slab for Memory<I> where I: StorageCoreInterface {
    fn slab_id (&self) -> slab::SlabId{
        self.slab_id.clone()
    }
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

        let counter = Rc::new(SlabCounter::new());
        let mut core = Rc::new(MemoryCore::new(
            slab_id,
            net.clone(),
            counter.clone(),
            dispatcher
        ));
        let dispatcher = Dispatcher::new( net.clone(), core.clone(), counter.clone() );

        let me = Memory{
            slab_id,
            core,
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