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
    peer_refs: HashMap<SlabId,SlabRef>,
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
            peer_refs:             HashMap::new(),
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
    pub fn put_slabref(&self, slab_id: SlabId, presence: &[SlabPresence] ) -> SlabRef {
        //println!("# Slab({}).put_slabref({}, {:?})", self.id, slab_id, presence );

        if slab_id == self.slab_id {
            // Don't even look it up if it's me. We must not allow any third party to edit the peering.
            // Also, my ref won't appear in the list of peer_refs, because it's not a peer
            return self.my_ref.clone();
        }

        let slabref = self.peer_refs.entry(slab_id).or_insert_with(|| SlabRef::new( slab_id, self.slab_id ));
        slabref.apply_presence(presence);
        return slabref.clone();
    }
}