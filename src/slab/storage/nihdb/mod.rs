mod worker;

use std::thread;

use context::*;
use network::{Network};
use slab::{self,Slab};
use slab::prelude::*;
use slab::counter::SlabCounter;

pub struct NIHDB{
    slab_id: slab::SlabId,
    worker_thread: thread::JoinHandle<()>,
    counters: SlabCounter,
    my_handle: LocalSlabHandle,
    my_ref: SlabRef,
    net: Network
}

impl Slab for NIHDB {
    fn slab_id (&self) -> slab::SlabId{
        self.slab_id.clone()
    }
    fn get_handle (&self) -> LocalSlabHandle {
        self.my_handle.clone()
    }
    fn get_slabref (&self) -> SlabRef {
       //self.slabref.clone()
        unimplemented!()
    }
    fn get_net (&self) -> Network {
        self.net.clone()
    }
    fn create_context (&self) -> Context {
        Context::new(self)
    }
}

impl NIHDB {
    pub fn new () -> Self {
        unimplemented!();
    }
}