mod worker;

use futures;
use futures::prelude::*;

// use subject::{SubjectId,SubjectType};
// use memorefhead::*;
use context::*;
use network::{Network,Transmitter,TransmitterArgs,TransportAddress};
use slabref::SlabRef;
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

pub struct MemorySlab{
    pub id: SlabId,
    worker_thread: thread::JoinHandle<()>,
    counters: SlabCounter,
    my_handle: SlabHandle,
    my_ref: SlabRef,
    net: Network
}

impl Slab for MemorySlab {
    fn get_handle (&self) -> SlabHandle {
        self.handle.clone()
    }
    fn get_ref (&self) -> SlabRef {
        self.my_ref.clone()
    }
    fn create_context (&self) -> Context {
        Context::new(self,self.net.clone())
    }
}

impl MemorySlab {
    pub fn new(net: &Network) -> Self {
        let slab_id = net.generate_slab_id();

        let my_ref = SlabRef::new(
            slab_id,
            slab_id, // I own my own ref to me, obviously
            Transmitter::new_blackhole(slab_id)
        );

        let (handle,handlestream) = SlabHandle::initialize( slab_id, my_ref );

        let counters = Arc::new(SlabCounter::new());

        let worker = MemorySlabWorker::new(
            slab_id,
            my_ref,
            net,
            counters.clone()
        );
        
        let worker_thread = thread::spawn(move || {
            let mut core = tokio_core::reactor::Core::new().unwrap();
            let server = handlestream.for_each(|(request, resp_channel)| {
                worker.dispatch_request(request,resp_channel);

                Ok(()) // keep accepting requests
            });

            core.run(server).unwrap();
        });


        let me = MemorySlab{
            slab_id,
            worker_thread,
            counters
        }

        net.register_local_slab(&me);
        net.conditionally_generate_root_index_seed(&me);

        me
    }
}

impl  Drop for MemorySlab {
    fn drop(&mut self) {
        self.dropping = true;

        //println!("# SlabInner({}).drop", self.id);
        self.memoref_dispatch_tx_channel.take();
        if let Some(t) = self.memoref_dispatch_thread.write().unwrap().take() {
            t.join().expect("join memoref_dispatch_thread");
        }
        self.net.deregister_local_slab(self.id);
        // TODO: Drop all observers? Or perhaps observers should drop the slab (weak ref directionality)
    }
}

impl fmt::Debug for MemorySlab {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Slab")
            .field("slab_id", &self.id)
            .finish()
    }
}