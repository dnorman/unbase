use std::rc::Rc;
use std::cell::RefCell;

use futures;

use super::core::MemoryCore;
use network::{Network};
use slab::{self, dispatcher::*, storage::*, prelude::*, Slab, counter::SlabCounter};
use context::Context;

use std::fmt;

pub struct Memory {
    slab_id: slab::SlabId,
    core: Rc<RefCell<MemoryCore>>,
    counter: Rc<SlabCounter>,
    dispatcher: Dispatcher,
    net: Network
}

impl Slab for Memory {
    fn slab_id (&self) -> slab::SlabId{
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

impl Memory {
    pub fn new(net: &Network) -> Self {
        let slab_id = net.generate_slab_id();


        let (dispatcher_tx, dispatcher_rx) =
            futures::unsync::mpsc::channel::<Dispatch>(1024);

        let counter = Rc::new(SlabCounter::new());
        let mut core = Rc::new(RefCell::new( MemoryCore::new(
            slab_id,
            net.clone(),
            counter.clone(),
            dispatcher_tx
        )));

        let dispatcher: Dispatcher = Dispatcher::new(net.clone(), core.clone(), dispatcher_rx );

        let me = Memory{
            slab_id,
            core,
            counter,
            dispatcher,
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