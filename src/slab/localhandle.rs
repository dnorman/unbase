use std::sync::Arc;
use futures::future;
use futures::prelude::*;
use futures::sync::{mpsc,oneshot};
use std::fmt;

use network;
use slab;
use slab::storage::StorageRequester;
use slab::prelude::*;
use slab::counter::SlabCounter;
use error::*;
use subject::{SubjectId,SubjectType};
use memorefhead::MemoRefHead;

impl LocalSlabHandle {
    pub fn new (slabref: SlabRef, counter: Arc<SlabCounter>, storage: StorageRequester) -> LocalSlabHandle {

        let handle = LocalSlabHandle{
            slab_id: slabref.slab_id(),
            slabref: slabref,
            counter,
            storage,
        };

        handle
    }

    pub fn get_memo (&self, memoref: MemoRef ) -> Box<Future<Item=Option<Memo>, Error=Error>> {
        self.storage.get_memo(memoref)
    }
    pub fn put_memo(&self, memo: Memo, peerstate: Vec<MemoPeerState>, from_slabref: SlabRef ) -> Box<Future<Item=(), Error=Error>> {
        self.storage.put_memo(memo, peerstate, from_slabref)
    }
    pub fn put_slab_presence(&self, presence: SlabPresence ) {
        self.storage.put_slab_presence(presence).wait();
    }
    pub fn get_peerstate(&self, memoref: MemoRef, maybe_dest_slabref: Option<SlabRef>) -> Result<Vec<MemoPeerState>, Error> {
        self.storage.get_peerstate(memoref, maybe_dest_slabref).wait()
    }

    pub fn slab_id(&self) -> slab::SlabId {
        self.slab_id.clone()
    }
    pub fn register_local_slabref(&self, peer_slab: &LocalSlabHandle) {

        //let args = TransmitterArgs::Local(&peer_slab);
        let presence = SlabPresence{
            slab_id: peer_slab.slab_id,
            addresses: vec![network::transport::TransportAddress::Local],
            lifetime: SlabAnticipatedLifetime::Unknown
        };

        self.put_slab_presence(presence);
    }
    pub fn get_slabref_for_slab_id( &self, slab_id: slab::SlabId ) -> SlabRef {
        // Temporary - SlabRef just contains SlabId for now. This shoulld change
        SlabRef{
            owning_slab_id: self.slab_id,
            slab_id: slab_id
        }
    }
    pub fn is_live (&self) -> bool {
        unimplemented!()
    }
    pub fn receive_memo_with_peerlist(&self, memo: Memo, peerlist: Vec<MemoPeerState>, from_slabref: SlabRef ) {
        unimplemented!();
        // self.call(LocalSlabRequest::ReceiveMemoWithPeerList{ memo, peerlist, from_slabref } ).wait()
    }

    fn assert_memoref( &self, memo_id: MemoId, subject_id: Option<SubjectId>, peerlist: Vec<MemoPeerState>, maybe_memo: Option<Memo>) -> (MemoRef, bool){
        unimplemented!()
    }
    pub fn remotize_memo_ids( &self, memo_ids: &[MemoId] ) -> Box<Future<Item=(), Error=Error>>  { 
        unimplemented!();
    }
    pub (crate) fn observe_subject (&self, subject_id: SubjectId, tx: mpsc::Sender<MemoRefHead> ) {

        unimplemented!()
        // let (tx,sub) = SubjectSubscription::new( subject_id, self.weak() );

        // match self.subject_subscriptions.lock().unwrap().entry(subject_id) {
        //     Entry::Vacant(e)   => {
        //         e.insert(vec![tx]);
        //     },
        //     Entry::Occupied(mut e) => {
        //         e.get_mut().push(tx);
        //     }
        // }

        // sub
    }
    pub (crate) fn observe_index (&self, tx: mpsc::Sender<MemoRefHead> ) {
        unimplemented!()
        // self.index_subscriptions.lock().unwrap().push(tx);
    }
    // pub fn unsubscribe_subject (&self){
    //     unimplemented!()
    //     // if let Some(subs) = self.subject_subscriptions.lock().unwrap().get_mut(&sub.subject_id) {
    //     //     subs.retain(|s| {
    //     //         s.cmp(&sub)
    //     //     });
    //     // }
    // }
    pub fn generate_subject_id(&self, stype: SubjectType) -> SubjectId {
        let id = (self.slab_id as u64).rotate_left(32) | self.counter.next_subject_id() as u64;
        SubjectId{ id, stype }
    }
    pub fn new_memo ( &self, subject_id: Option<SubjectId>, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        let memo_id = (self.slab_id as u64).rotate_left(32) | self.counter.next_memo_id() as u64;

        //println!("# Slab({}).new_memo(id: {},subject_id: {:?}, parents: {:?}, body: {:?})", self.slab_id, memo_id, subject_id, parents.memo_ids(), body );

        let memo = Memo {
            id:    memo_id,
            owning_slabref: self.slabref.clone(),
            subject_id: subject_id,
            parents: parents,
            body: body
        };

        // TODO1 figure out if put_memo should give back a remoref. Probably Yes??
        unimplemented!();
        //let (memoref, _had_memoref) = self.put_memo( memo, vec![], self.slabref ).wait();

        // TODO1 figure out where and how this should be called
        //self.consider_emit_memo(&memoref);

        //memoref
    }
    pub fn residentize_memoref(&self, memoref: &MemoRef, memo: Memo) -> bool {
        //println!("# Slab({}).MemoRef({}).residentize()", self.slab_id, memoref.id);

        assert!(memoref.owning_slab_id == self.slab_id);
        assert!( memoref.memo_id() == memo.id );

        // TODO get rid of ptr, and possibly the whole function
        let mut ptr = memoref.ptr.write().unwrap();

        if let MemoRefPtr::Remote = *ptr {
            *ptr = MemoRefPtr::Resident( Arc::new(memo) );

            // should this be using do_peering_for_memo?
            // doing it manually for now, because I think we might only want to do
            // a concise update to reflect our peering status change

            let peering_memoref = self.new_memo(
                None,
                memoref.to_head(),
                MemoBody::Peering(
                    memoref.memo_id(),
                    memoref.subject_id,
                    vec![ MemoPeerState{
                        slabref: self.slabref.clone(),
                        status: MemoPeerStatus::Resident
                    }]
                )
            );

            //TODO1
            unimplemented!();
            // let requests = Vec::new();
            // for peer in memoref.peerstate.read().unwrap().iter() {

            //     requests.push( self.call(LocalSlabRequest::SendMemo{ slabref: peer.slab_id, memoref: peering_memoref.clone() } ) );
            //     peer.slabref.send( &self.slabref, &peering_memoref );
            // }

            // residentized
            true
        }else{
            // already resident
            false
        }
    }
    pub fn remotize_memoref( &self, memoref: &MemoRef ) -> Result<(),Error> {
        assert!(memoref.owning_slab_id == self.slab_id);

        //println!("# Slab({}).MemoRef({}).remotize()", self.slab_id, memoref.id );
        
        // TODO: check peering minimums here, and punt if we're below threshold

        // TODO1
        unimplemented!()
        // self.store.conditional_remove_memo( memoref.id )?;

        // let peering_memoref = self.new_memo_basic(
        //     None,
        //     memoref.to_head(),
        //     MemoBody::Peering(
        //         memoref.id,
        //         memoref.subject_id,
        //         vec![MemoPeerState{
        //             slabref: self.slabref.clone(),
        //             status: MemoPeerStatus::Participating
        //         }]
        //     )
        // );

        //self.consider_emit_memo(&memoref);

        // for peer in memoref.peerlist.iter() {
        //     peer.slabref.send( &self.my_ref, &peering_memoref );
        // }

        // Ok(())
    }
    pub fn request_memo (&self, memoref: &MemoRef) -> u8 {
        //println!("Slab({}).request_memo({})", self.slab_id, memoref.id );

        let request_memo = self.new_memo_basic(
            None,
            MemoRefHead::Null,
            MemoBody::MemoRequest(
                vec![memoref.id],
                self.presence()
            )
        );

        let mut sent = 0u8;
        for peer in memoref.peerlist.read().unwrap().iter().take(5) {
            //println!("Slab({}).request_memo({}) from {}", self.slab_id, memoref.id, peer.slabref.slab_id );
            peer.slabref.send( &self.my_ref, &request_memo.clone() );
            sent += 1;
        }

        sent
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

                        origin_slabref.send( &self.my_ref, &my_presence_memoref );

                        let _ = mentioned_slabref;
                        // needs PartialEq
                        //if mentioned_slabref != origin_slabref {
                        //   mentioned_slabref.send( &self.my_ref, &my_presence_memoref );
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
                                requesting_slabref.send(&self.my_ref, &desired_memoref)
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
                                        slabref: self.my_ref.clone(),
                                        status: MemoPeerStatus::NonParticipating
                                    }]
                                )
                            );
                            requesting_slabref.send(&self.my_ref, &peering_memoref)
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
    pub fn count_of_memorefs_resident( &self ) -> u32 {
        unimplemented!()
        //self.memorefs_by_id.read().unwrap().len() as u32
    }
    pub fn count_of_memos_received( &self ) -> u64 {
        self.counter.get_memos_received() as u64
    }
    pub fn count_of_memos_reduntantly_received( &self ) -> u64 {
        self.counter.get_memos_redundantly_received() as u64
    }
    pub fn peer_slab_count (&self) -> usize {
        self.counter.get_peer_slabs() as usize
    }
    pub fn new_memo_basic (&self, subject_id: Option<SubjectId>, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        self.new_memo(subject_id, parents, body)
    }
    pub fn new_memo_basic_noparent (&self, subject_id: Option<SubjectId>, body: MemoBody) -> MemoRef {
        self.new_memo(subject_id, MemoRefHead::Null, body)
    }
    // should this be a function of the slabref rather than the owning slab?
    pub fn presence_for_origin (&self, origin_slabref: &SlabRef ) -> SlabPresence {
        // Get the address that the remote slab would recogize
        unimplemented!()
        // SlabPresence {
        //     slab_id: self.slab_id,
        //     addresses: origin_slabref.get_return_addresses(),
        //     lifetime: SlabAnticipatedLifetime::Unknown
        // }
    }
    // pub fn slabhandle_from_presence(&self, presence: &SlabPresence) -> Result<SlabHandle,Error> {
    //         match presence.address {
    //             TransportAddress::Simulator | TransportAddress::Local  => {
    //                 return Err(Error::StorageOpDeclined(StorageOpDeclined::InvalidAddress))
    //             }
    //             _ => { }
    //         };


    //     //let args = TransmitterArgs::Remote( &presence.slab_id, &presence.address );
    //     presence.get_transmitter(&self.net);

    //     Ok(self.put_slabref( presence.slab_id, &vec![presence.clone()] ))
    // }



        //             if slab.request_memo(self) > 0 {
        //         channel = slab.memo_wait_channel(self.slab_id);
        //     }else{
        //         return Err(Error::RetrieveError(RetrieveError::NotFound))
        //     }

        // // By sending the memo itself through the channel
        // // we guarantee that there's no funny business with request / remotize timing


        // use std::time;
        // let timeout = time::Duration::from_millis(100000);

        // for _ in 0..3 {
        //     match channel.recv_timeout(timeout) {
        //         Ok(memo)       =>{
        //             //println!("Slab({}).MemoRef({}).get_memo() received memo: {}", self.owning_slab_id, self.slab_id, memo.id );
        //             return Ok(memo)
        //         }
        //         Err(rcv_error) => {

        //             use std::sync::mpsc::RecvTimeoutError::*;
        //             match rcv_error {
        //                 Timeout => {}
        //                 Disconnected => {
        //                     return Err(Error::RetrieveError(RetrieveError::SlabError))
        //                 }
        //             }
        //         }
        //     }

        //     // have another go around
        //     if slab.request_memo( &self ) == 0 {
        //         return Err(Error::RetrieveError(RetrieveError::NotFound))
        //     }

        // }

        // Err(Error::RetrieveError(RetrieveError::NotFoundByDeadline))

    // pub fn remotize_memo_ids_wait( &self, memo_ids: &[MemoId], ms: u64 ) -> Result<(),Error> {
    //     use std::time::{Instant,Duration};
    //     let start = Instant::now();
    //     let wait = Duration::from_millis(ms);
    //     use std::thread;

    //     loop {
    //         if start.elapsed() > wait{
    //             return Err(Error::StorageOpDeclined(StorageOpDeclined::InsufficientPeering))
    //         }

    //         #[allow(unreachable_patterns)]
    //         match self.call(LocalSlabRequest::RemotizeMemoIds{ memo_ids } ).wait() {
    //             Ok(_) => {
    //                 return Ok(())
    //             },
    //             Err(Error::StorageOpDeclined(StorageOpDeclined::InsufficientPeering)) => {}
    //             Err(e)                                      => return Err(e)
    //         }

    //         thread::sleep(Duration::from_millis(50));
    //     }
    // }
}


impl fmt::Debug for LocalSlabHandle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("LocalSlabHandle")
            .field("slab_id", &self.slab_id)
            .finish()
    }
}