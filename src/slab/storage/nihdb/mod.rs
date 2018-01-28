mod worker;

use std::thread;

use context::*;
use network::{Network};
use slab::Slab;
use slab::prelude::*;
use slab::counter::SlabCounter;

pub struct NIHDB{
    pub id: SlabId,
    worker_thread: thread::JoinHandle<()>,
    counters: SlabCounter,
    my_handle: LocalSlabHandle,
    my_ref: SlabRef,
    net: Network
}

impl Slab for NIHDB {
    fn get_handle (&self) -> LocalSlabHandle {
        self.my_handle.clone()
    }
    fn get_ref (&self) -> SlabRef {
        self.my_ref.clone()
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