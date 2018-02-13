use std::collections::{HashMap, hash_map::Entry};
use std::sync::Arc;
use futures::{ future, prelude::*, sync::{mpsc,oneshot} };
use tokio::executor::current_thread;
use std::thread;

use slab::{prelude::*, storage::*, counter::SlabCounter};
use subject::{SubjectId,SubjectType};
use memorefhead::MemoRefHead;

pub struct MemoDispatch{
    memo: Memo,
    memoref: MemoRef,
    from_slabref: SlabRef
}

pub struct Dispatcher{
    pub tx: mpsc::UnboundedSender<MemoDispatch>,
    worker_thread: thread::JoinHandle<()>
}

struct DispatcherInner{
    storage: StorageRequester,

    memo_wait_channels: HashMap<MemoId,Vec<oneshot::Sender<Memo>>>,
    subject_subscriptions: HashMap<SubjectId, Vec<mpsc::Sender<MemoRefHead>>>,
    index_subscriptions: Vec<mpsc::Sender<MemoRefHead>>,
}

impl Dispatcher{
    pub fn new ( storage: StorageRequester, counter: Arc<SlabCounter> ) -> Dispatcher {

        let (tx,rx) = mpsc::unbounded::<MemoDispatch>();

        let inner = DispatcherInner{
            storage:               storage,
            memo_wait_channels:    HashMap::new(),
            subject_subscriptions: HashMap::new(),
            index_subscriptions:   Vec::new(),
        };

        let worker_thread = thread::spawn(move || {
            current_thread::run(|_| {

            let server = rx.for_each(|dispatch| {
                if let MemoDispatch{ memo, memoref, from_slabref } = dispatch {
                    current_thread::spawn( inner.dispatch(memo, memoref, from_slabref) );
                }
                future::result(Ok(()))
            });

            current_thread::spawn(server);

            });
        });

        Dispatcher{ tx, worker_thread }
    }
}

impl DispatcherInner{
    pub fn dispatch(&self, memo: Memo, memoref: MemoRef, from_slabref: SlabRef) -> Box<Future<Item=(), Error=()>> {

        // self.check_memo_waiters(&memo);
        // self.consider_emit_memo(&memo);
        // self.handle_memo_from_other_slab();
        // self.do_peering(&memoref, from_slabref);
        // self.dispatch_memoref(memoref);

        Box::new(future::result(Ok(())))
    }

    pub fn check_memo_waiters ( &self, memo: &Memo) {
        match self.memo_wait_channels.entry(memo.id) {
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

    /// Notify interested parties about a newly arrived memoref on this slab
    pub fn dispatch_memoref (&self, memoref : MemoRef){
        //println!("# \t\\ Slab({}).dispatch_memoref({}, {:?}, {:?})", self.slab_id, &memoref.id, &memoref.subject_id, memoref.get_memo_if_resident() );

        if let Some(subject_id) = memoref.subject_id {
            // TODO: Switch subject subscription mechanism to be be based on channels, and matching trees
            // subject_subscriptions: Mutex<HashMap<SubjectId, Vec<mpsc::Sender<Option<MemoRef>>>>>


            if let SubjectType::IndexNode = subject_id.stype {
                // TODO3 - update this to consider popularity of this node, and/or common points of reference with a given context
                let mut senders = self.index_subscriptions.lock().unwrap();
                let len = senders.len();
                for i in (0..len).rev() {
                    if let Err(_) = senders[i].clone().send(memoref.to_head()).wait(){
                        // TODO3: proactively remove senders when the receiver goes out of scope. Necessary for memory bloat
                        senders.swap_remove(i);
                    }
                }

            }

            if let Some(ref mut senders) = self.subject_subscriptions.lock().unwrap().get_mut( &subject_id ) { 
                let len = senders.len();

                for i in (0..len).rev() {
                    match senders[i].clone().send(memoref.to_head()).wait() {
                        Ok(..) => { }
                        Err(_) => {
                            // TODO3: proactively remove senders when the receiver goes out of scope. Necessary for memory bloat
                            senders.swap_remove(i);
                        }
                    }
                }
            }

        }
    }

    //NOTE: nothing that calls get_memo, directly or indirectly is presently allowed here (but get_memo_if_resident is ok)
    //      why? Presumably due to deadlocks, but this seems sloppy
    /// Perform necessary tasks given a newly arrived memo on this slab
    pub fn handle_memo_from_other_slab( &self, memo: &Memo, memoref: &MemoRef, origin_slabref: &SlabRef ){
        //println!("Slab({}).handle_memo_from_other_slab({:?})", self.slab_id, memo );

        match memo.body {
            // This Memo is a peering status update for another memo
            MemoBody::SlabPresence{ p: ref presence, r: ref root_index_seed } => {

                match root_index_seed {
                    &MemoRefHead::Subject{..} | &MemoRefHead::Anonymous{..} => {
                        // HACK - this should be done inside the deserialize
                        for memoref in root_index_seed.iter() {
                            memoref.update_peer(origin_slabref, MemoPeerStatus::Resident);
                        }

                        self.net.apply_root_index_seed( &presence, root_index_seed, self );
                    }
                    &MemoRefHead::Null => {}
                }

                let mut reply = false;
                if let &MemoRefHead::Null = root_index_seed {
                    reply = true;
                }

                if reply {
                    if let Ok(mentioned_slabref) = self.slabref_from_presence( presence ) {
                        // TODO: should we be telling the origin slabref, or the presence slabref that we're here?
                        //       these will usually be the same, but not always

                        let my_presence_memoref = self.new_memo_basic(
                            None,
                            memoref.to_head(),
                            MemoBody::SlabPresence{
                                p: self.presence_for_origin( origin_slabref ),
                                r: self.get_root_index_seed()
                            }
                        );

                        origin_slabref.send( &self.slabref, &my_presence_memoref );

                        let _ = mentioned_slabref;
                        // needs PartialEq
                        //if mentioned_slabref != origin_slabref {
                        //   mentioned_slabref.send( &self.slabref, &my_presence_memoref );
                        //}
                    }
                }
            }
            MemoBody::Peering(memo_id, subject_id, ref peerlist ) => {
                let (peered_memoref,_had_memo) = self.assert_memoref( memo_id, subject_id, peerlist.clone() );

                // Don't peer with yourself
                for peer in peerlist.iter().filter(|p| p.slabref != self.slabref ) {
                    peered_memoref.update_peer( &peer.slabref, peer.status.clone());
                }

                if 0 == peered_memoref.want_peer_count() {
                    self.remove_from_durability_remediation(peered_memoref);
                }
            },
            MemoBody::MemoRequest(ref desired_memo_ids, ref requesting_slabref ) => {

                if requesting_slabref.0.slab_id != self.slab_id {
                    for desired_memo_id in desired_memo_ids {
                        if let Ok(Some(desired_memoref)) = self.store.get_memoref(&desired_memo_id) {

                            if desired_memoref.is_resident() {
                                requesting_slabref.send(&self.slabref, &desired_memoref)
                            } else {
                                // Somebody asked me for a memo I don't have
                                // It would be neighborly to tell them I don't have it
                                self.do_peering(&memoref,requesting_slabref);
                            }
                        }else{
                            let peering_memoref = self.new_memo(
                                None,
                                memoref.to_head(),
                                MemoBody::Peering(
                                    *desired_memo_id,
                                    None,
                                    vec![MemoPeerState{
                                        slabref: self.slabref.clone(),
                                        status: MemoPeerStatus::NonParticipating
                                    }]
                                )
                            );
                            requesting_slabref.send(&self.slabref, &peering_memoref)
                        }
                    }
                }
            }
            // _ => {
            //     if let Some(SubjectId{stype: SubjectType::IndexNode,..}) = memo.subject_id {
            //         for slab in self.contexts {

            //         }
            //     }
            // }
            _ => {}
        }
    }
    /// Conditionally emit memo for durability assurance
    pub fn consider_emit_memo(&self, memoref: &MemoRef) {
        // At present, some memos like peering and slab presence are emitted manually.
        // TODO: This will almost certainly have to change once gossip/plumtree functionality is added

        let needs_peers = memoref.want_peer_count();

        if needs_peers > 0 {
            self.add_to_durability_remediation(memoref);

            for peer_ref in self.peer_refs.read().unwrap().iter().filter(|x| !memoref.is_peered_with_slabref(x) ).take( needs_peers as usize ) {
                peer_ref.send( &self.slabref, memoref );
            }
        }else{
            self.remove_from_durability_remediation(&memoref);
        }
    }

        /// Add memorefs which have been deemed as under-replicated to the durability remediation queue
    fn add_to_durability_remediation(&self, memoref: &MemoRef){

        // TODO: transition this to a crossbeam_channel for add/remove and update the remediation thread to manage the list
        let mut rem_q = self.peering_remediation_queue.lock().unwrap();
        if !rem_q.contains(&memoref) {
            rem_q.push(memoref.clone());
        }
    }
    fn remove_from_durability_remediation(&self, memoref: &MemoRef){
        let mut q = self.peering_remediation_queue.lock().unwrap();
        q.retain(|mr| mr != memoref )
    }
}