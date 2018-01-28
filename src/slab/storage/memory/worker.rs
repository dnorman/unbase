use std::thread;
use std::collections::HashMap;
use std::sync::Arc;
use futures::prelude::*;
use futures::sync::{mpsc,oneshot};
use tokio_core;

use subject::SubjectId;
use network::Network;
use slab::prelude::*;
use slab::counter::SlabCounter;
use memorefhead::MemoRefHead;

struct MemoCarrier{
    memo:    Option<Memo>,
    memoref: Option<MemoRef>,
}

pub struct MemoryWorker {
    pub slab_id: SlabId,
    counter: Arc<SlabCounter>,
    memo_storage: HashMap<MemoId,MemoCarrier>,

    memo_wait_channels: HashMap<MemoId,Vec<oneshot::Sender<Memo>>>,
    subject_subscriptions: HashMap<SubjectId, Vec<mpsc::Sender<MemoRefHead>>>,
    index_subscriptions: Vec<mpsc::Sender<MemoRefHead>>,
    peer_refs: Vec<SlabRef>,
    // peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    // peering_remediation_queue: Mutex<Vec<MemoRef>>,

    pub my_ref: SlabRef,
    net: Network
}


impl MemoryWorker {
    pub fn spawn ( slab_id: SlabId, my_ref: SlabRef, net: Network, counter: Arc<SlabCounter> ) -> (mpsc::UnboundedSender<(SlabRequest,oneshot::Sender<SlabResponse>)>, thread::JoinHandle<()>) {
        let me = MemoryWorker{
            slab_id,
            my_ref,
            net,
            counter,

            memo_storage:          HashMap::new(),
            memo_wait_channels:    HashMap::new(),
            subject_subscriptions: HashMap::new(),
            index_subscriptions:   Vec::new(),
            peer_refs:             Vec::new(),
        };

        let (tx,rx) = mpsc::unbounded();

        let worker_thread = thread::spawn(move || {
            let mut core = tokio_core::reactor::Core::new().unwrap();
            let server = rx.for_each(|(request, resp_channel)| {
                me.dispatch_request(request,resp_channel);

                Ok(()) // keep accepting requests
            });

            core.run(server).unwrap();
        });

        (tx,worker_thread)
    }
    fn dispatch_request(&self,request: SlabRequest, responder: oneshot::Sender<SlabResponse>) {
        unimplemented!()
    }
}