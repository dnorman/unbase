
pub use self::common_structs::*;
pub use self::slabref::{SlabRef,SlabRefInner};
pub use self::memoref::{MemoRef,MemoRefInner,MemoRefPtr};
pub use self::memo::{MemoId,Memo,MemoInner,MemoBody};
pub use self::memoref::serde as memoref_serde;
pub use self::memo::serde as memo_serde;

use crate::subject::{SubjectId,SubjectType};
use crate::memorefhead::*;
use crate::context::*;
use crate::network::{Network,Transmitter,TransmitterArgs,TransportAddress};

use std::ops::Deref;
use std::sync::{Arc,Weak,RwLock,Mutex};
use std::sync::mpsc;
use std::sync::mpsc::channel;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::fmt;
use std::thread;
use std::time;

// NOTE: All slab code is broken down into functional areas:
mod ingress;
mod egress;
mod core;
mod convenience;
mod devutils;
mod common_structs;
mod memo;
mod slabref;
mod memoref;

pub type SlabId = u32;

#[derive(Clone)]
pub struct Slab(Arc<SlabInner>);

impl Deref for Slab {
    type Target = SlabInner;
    fn deref(&self) -> &SlabInner {
        &*self.0
    }
}

pub struct SlabInner{
    pub id: SlabId,
    memorefs_by_id: RwLock<HashMap<MemoId,MemoRef>>,
    memo_wait_channels: Mutex<HashMap<MemoId,Vec<mpsc::Sender<Memo>>>>, // TODO: HERE HERE HERE - convert to per thread wait channel senders?
    subject_subscriptions: Mutex<HashMap<SubjectId, Vec<futures::sync::mpsc::Sender<MemoRefHead>>>>,
    index_subscriptions: Mutex<Vec<futures::sync::mpsc::Sender<MemoRefHead>>>,
    counters: RwLock<SlabCounters>,

    memoref_dispatch_tx_channel: Option<Mutex<mpsc::Sender<MemoRef>>>,
    memoref_dispatch_thread: RwLock<Option<thread::JoinHandle<()>>>,

    peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    peering_remediation_queue: Mutex<Vec<MemoRef>>,

    pub my_ref: SlabRef,
    peer_refs: RwLock<Vec<SlabRef>>,
    net: Network,
    pub dropping: bool
}

struct SlabCounters{
    last_memo_id: u32,
    last_subject_id: u32,
    memos_received: u64,
    memos_redundantly_received: u64,
}

#[derive(Clone)]
pub struct WeakSlab{
    pub id: u32,
    inner: Weak<SlabInner>
}

impl Slab {
    pub fn new(net: &Network) -> Slab {
        let slab_id = net.generate_slab_id();

        let my_ref_inner = SlabRefInner {
            slab_id: slab_id,
            owning_slab_id: slab_id, // I own my own ref to me, obviously
            presence: RwLock::new(vec![]), // this bit is just for show
            tx: Mutex::new(Transmitter::new_blackhole(slab_id)),
            return_address: RwLock::new(TransportAddress::Local),
        };

        let my_ref = SlabRef(Arc::new(my_ref_inner));
        // TODO: figure out how to reconcile this with the simulator
        let (memoref_dispatch_tx_channel, memoref_dispatch_rx_channel) = mpsc::channel::<MemoRef>();

        let inner = SlabInner {
            id: slab_id,
            memorefs_by_id:        RwLock::new(HashMap::new()),
            memo_wait_channels:    Mutex::new(HashMap::new()),
            subject_subscriptions: Mutex::new(HashMap::new()),
            index_subscriptions:   Mutex::new(Vec::new()),

            counters: RwLock::new(SlabCounters {
                last_memo_id: 5000,
                last_subject_id: 9000,
                memos_received: 0,
                memos_redundantly_received: 0,
            }),

            memoref_dispatch_tx_channel: Some(Mutex::new(memoref_dispatch_tx_channel)),
            memoref_dispatch_thread: RwLock::new(None),
            peering_remediation_thread: RwLock::new(None),
            peering_remediation_queue: Mutex::new(Vec::new()),

            my_ref: my_ref,
            peer_refs: RwLock::new(Vec::new()),
            net: net.clone(),
            dropping: false
        };

        let me = Slab(Arc::new(inner));
        net.register_local_slab(&me);

        let weak_self = me.weak();

        // TODO: this should really be a thread pool, or get_memo should be changed to be nonblocking somhow
        *me.memoref_dispatch_thread.write().unwrap() = Some(thread::spawn(move || {
            while let Ok(memoref) = memoref_dispatch_rx_channel.recv() {
                if let Some(slab) = weak_self.upgrade(){
                    slab.dispatch_memoref(memoref);
                }
            }
        }));

        let weak_self = me.weak();

        // TODO: this should really be a thread pool, or get_memo should be changed to be nonblocking somhow
        *me.peering_remediation_thread.write().unwrap() = Some(thread::spawn(move || {
            loop {
                thread::sleep( time::Duration::from_millis(50) );
                //println!("PEERING REMEDIATION");
                if let Some(slab) = weak_self.upgrade(){
                    // TODO - Get rid of this clone. did it as a cheap way to avoid the deadlock below
                    let q = { slab.peering_remediation_queue.lock().unwrap().clone() }; 
                    for memoref in q {
                        // Kind of dumb that it's checking 
                         slab.consider_emit_memo(&memoref);
                    }
                }
            }
        }));

        net.conditionally_generate_root_index_seed(&me);

        me
    }
    pub fn weak (&self) -> WeakSlab {
        WeakSlab {
            id: self.id,
            inner: Arc::downgrade(&self.0)
        }
    }
    pub fn get_root_index_seed (&self) -> MemoRefHead {
        self.net.get_root_index_seed(self)
    }
    pub fn create_context (&self) -> Context {
        Context::new(self)
    }
    pub (crate) fn observe_subject (&self, subject_id: SubjectId, tx: futures::sync::mpsc::Sender<MemoRefHead> ) {

        // let (tx,sub) = SubjectSubscription::new( subject_id, self.weak() );

        match self.subject_subscriptions.lock().unwrap().entry(subject_id) {
            Entry::Vacant(e)   => {
                e.insert(vec![tx]);
            },
            Entry::Occupied(mut e) => {
                e.get_mut().push(tx);
            }
        }

        // sub
    }
    pub (crate) fn observe_index (&self, tx: futures::sync::mpsc::Sender<MemoRefHead> ) {
        self.index_subscriptions.lock().unwrap().push(tx);
    }
    // pub fn unsubscribe_subject (&self){
    //     unimplemented!()
    //     // if let Some(subs) = self.subject_subscriptions.lock().unwrap().get_mut(&sub.subject_id) {
    //     //     subs.retain(|s| {
    //     //         s.cmp(&sub)
    //     //     });
    //     // }
    // }
    pub fn memo_wait_channel (&self, memo_id: MemoId ) -> mpsc::Receiver<Memo> {
        let (tx, rx) = channel::<Memo>();

        match self.memo_wait_channels.lock().unwrap().entry(memo_id) {
            Entry::Vacant(o)       => { o.insert( vec![tx] ); }
            Entry::Occupied(mut o) => { o.get_mut().push(tx); }
        };

        rx
    }
    pub fn generate_subject_id(&self, stype: SubjectType) -> SubjectId {
        let mut counters = self.counters.write().unwrap();
        counters.last_subject_id += 1;
        let id = (self.id as u64).rotate_left(32) | counters.last_subject_id as u64;
        SubjectId{ id, stype }
    }
    pub fn check_memo_waiters ( &self, memo: &Memo) {
        match self.memo_wait_channels.lock().unwrap().entry(memo.id) {
            Entry::Occupied(o) => {
                for channel in o.get() {
                    // we don't care if it worked or not.
                    // if the channel is closed, we're scrubbing it anyway
                    channel.send(memo.clone()).ok();
                }
                o.remove();
            },
            Entry::Vacant(_) => {}
        };
    }

    pub fn do_peering(&self, memoref: &MemoRef, origin_slabref: &SlabRef) {

        let do_send = if let Some(memo) = memoref.get_memo_if_resident(){
            // Peering memos don't get peering memos, but Edit memos do
            // Abstracting this, because there might be more types that don't do peering
            memo.does_peering()
        }else{
            // we're always willing to do peering for non-resident memos
            true
        };

        if do_send {
            // That we received the memo means that the sender didn't think we had it
            // Whether or not we had it already, lets tell them we have it now.
            // It's useful for them to know we have it, and it'll help them STFU

            // TODO: determine if peering memo should:
            //    A. use parents at all
            //    B. and if so, what should be should we be using them for?
            //    C. Should we be sing that to determine the peered memo instead of the payload?
            //println!("MEOW {}, {:?}", my_ref );

            let peering_memoref = self.new_memo(
                None,
                memoref.to_head(),
                MemoBody::Peering(
                    memoref.id,
                    memoref.subject_id,
                    memoref.get_peerlist_for_peer(&self.my_ref, Some(origin_slabref.slab_id))
                )
            );
            origin_slabref.send( &self.my_ref, &peering_memoref );
        }

    }
}

impl Drop for SlabInner {
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

impl WeakSlab {
    pub fn upgrade (&self) -> Option<Slab> {
        match self.inner.upgrade() {
            Some(i) => Some( Slab(i) ),
            None    => None
        }
    }
}