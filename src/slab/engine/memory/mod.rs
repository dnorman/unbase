
mod store;
use self::store::MemoryStore;

use network::{Network};
use slab::{self, dispatcher::*, storage::*, prelude::*, Slab, counter::SlabCounter};
use context::Context;

use actix::prelude::*;
use futures;
use std::fmt;
use std::rc::Rc;

pub struct MemoryEngine<S> where S: Handler<StorageRequest> {
    slab_id: slab::SlabId,
    store: Addr<Unsync,S>,
    counter: Rc<SlabCounter>,
    dispatcher: Dispatcher<S>,
    net: Network
}

impl <S> Slab for MemoryEngine<S> {
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

impl <S> MemoryEngine<S>  where S: Handler<StorageRequest> {
    pub fn new(net: &Network) -> Self {
        let slab_id = net.generate_slab_id();

        let (dispatcher_tx, dispatcher_rx) =
            futures::unsync::mpsc::channel::<Dispatch>(1024);

        let counter = Rc::new(SlabCounter::new());
        let store: Addr<Unsync,MemoryStore> = MemoryStore::new(
            slab_id,
            net.clone(),
            counter.clone(),
            dispatcher_tx
        ).start();

        let dispatcher = Dispatcher::<MemoryStore>::new(net.clone(), store.clone(), dispatcher_rx );

        let me = MemoryEngine {
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
}

impl Drop for MemoryEngine {
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

impl fmt::Debug for MemoryEngine {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Slab")
            .field("slab_id", &self.slab_id)
            .finish()
    }
}