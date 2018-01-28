use std::sync::Arc;
use core::ops::Deref;
use futures::*;
use futures::sync::{mpsc,oneshot};
use std::fmt;

use network;
use slab::prelude::*;
use error::*;
use subject::{SubjectId,SubjectType};
use memorefhead::MemoRefHead;

impl LocalSlabHandle {
    pub fn new (slab_id: SlabId, tx: mpsc::UnboundedSender<(LocalSlabRequest,oneshot::Sender<LocalSlabResponse>)>) -> LocalSlabHandle {

        let handle = LocalSlabHandle{
            id: slab_id,
            tx,
        };

        handle
    }

    fn call (&self, request: LocalSlabRequest ) -> Box<Future<Item=LocalSlabResponse, Error=Error>> {
        let (p, c) = oneshot::channel::<LocalSlabResponse>();
        unimplemented!()
    }
    pub fn register_local_slabref(&self, peer_slab: &LocalSlabHandle) {

        //let args = TransmitterArgs::Local(&peer_slab);
        let presence = SlabPresence{
            slab_id: peer_slab.id,
            address: network::transport::TransportAddress::Local,
            lifetime: SlabAnticipatedLifetime::Unknown
        };

        self.put_slab_presence(&vec![presence]);
    }
    pub fn is_live (&self) -> bool {
        unimplemented!()
    }
    pub fn receive_memo_with_peerlist(&self, memo: Memo, peerlist: MemoPeerList, from_slabref: SlabRef ) {
        unimplemented!();
        // self.call(LocalSlabRequest::ReceiveMemoWithPeerList{ memo, peerlist, from_slabref } ).wait()
    }
    pub fn put_slab_presence(&self, slab_id: SlabId, presence: &[SlabPresence] ) { 
       unimplemented!();

        // self.call(LocalSlabRequest::PutSlabPresence{ slab_id, presence } ).wait()?
    }
    fn assert_memoref( &self, memo_id: MemoId, subject_id: Option<SubjectId>, peerlist: MemoPeerList, maybe_memo: Option<Memo>) -> (MemoRef, bool){
        unimplemented!()
    }
    pub fn remotize_memo_ids( &self, memo_ids: &[MemoId] ) -> Box<Future<Item=(), Error=Error>>  { 
                unimplemented!();

            // self.call(LocalSlabRequest::RemotizeMemoIds{ memo_ids } ).wait()?
    }
    pub fn get_memoref (&self, memo_id: MemoId ) -> Box<Future<Item=Memo, Error=Error>> {
        unimplemented!();
    }
    pub fn get_memo (&self, memo_id: MemoId ) -> Box<Future<Item=Memo, Error=Error>> {
        unimplemented!();

        // if let LocalSlabResponse::GetMemo(memo) = self.call(LocalSlabRequest::GetMemo{ memo_id } ) {

        // }
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
        let mut counters = self.counter.write().unwrap();
        counters.last_subject_id += 1;
        let id = (self.id as u64).rotate_left(32) | counters.last_subject_id as u64;
        SubjectId{ id, stype }
    }
    pub fn new_memo ( &self, subject_id: Option<SubjectId>, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        let memo_id = (self.id as u64).rotate_left(32) | self.counters.last_memo_id() as u64;

        //println!("# Slab({}).new_memo(id: {},subject_id: {:?}, parents: {:?}, body: {:?})", self.id, memo_id, subject_id, parents.memo_ids(), body );

        let memo = Memo::new(MemoInner {
            id:    memo_id,
            owning_slab_id: self.id,
            subject_id: subject_id,
            parents: parents,
            body: body
        });

        let (memoref, _had_memoref) = self.put_memo( memo );
        self.consider_emit_memo(&memoref);

        memoref
    }
    pub fn residentize_memoref(&self, memoref: &MemoRef, memo: Memo) -> bool {
        //println!("# Slab({}).MemoRef({}).residentize()", self.id, memoref.id);

        assert!(memoref.owning_slab_id == self.id);
        assert!( memoref.id == memo.id );

        let mut ptr = memoref.ptr.write().unwrap();

        if let MemoRefPtr::Remote = *ptr {
            *ptr = MemoRefPtr::Resident( memo );

            // should this be using do_peering_for_memo?
            // doing it manually for now, because I think we might only want to do
            // a concise update to reflect our peering status change

            let peering_memoref = self.new_memo(
                None,
                memoref.to_head(),
                MemoBody::Peering(
                    memoref.id,
                    memoref.subject_id,
                    MemoPeerList::new(vec![ MemoPeer{
                        slabref: self.my_ref.clone(),
                        status: MemoPeeringStatus::Resident
                    }])
                )
            );

            for peer in memoref.peerlist.read().unwrap().iter() {
                peer.slabref.send( &self.my_ref, &peering_memoref );
            }

            // residentized
            true
        }else{
            // already resident
            false
        }
    }
    pub fn remotize_memoref( &self, memoref: &MemoRef ) -> Result<(),Error> {
        assert!(memoref.owning_slab_id == self.id);

        //println!("# Slab({}).MemoRef({}).remotize()", self.id, memoref.id );
        
        // TODO: check peering minimums here, and punt if we're below threshold

        self.store.conditional_remove_memo( memoref.id )?;

        let peering_memoref = self.new_memo_basic(
            None,
            memoref.to_head(),
            MemoBody::Peering(
                memoref.id,
                memoref.subject_id,
                MemoPeerList::new(vec![MemoPeer{
                    slabref: self.my_ref.clone(),
                    status: MemoPeeringStatus::Participating
                }])
            )
        );

        //self.consider_emit_memo(&memoref);

        for peer in memoref.peerlist.iter() {
            peer.slabref.send( &self.my_ref, &peering_memoref );
        }

        Ok(())
    }
    pub fn request_memo (&self, memoref: &MemoRef) -> u8 {
        //println!("Slab({}).request_memo({})", self.id, memoref.id );

        let request_memo = self.new_memo_basic(
            None,
            MemoRefHead::Null,
            MemoBody::MemoRequest(
                vec![memoref.id],
                self.my_ref.clone()
            )
        );

        let mut sent = 0u8;
        for peer in memoref.peerlist.read().unwrap().iter().take(5) {
            //println!("Slab({}).request_memo({}) from {}", self.id, memoref.id, peer.slabref.slab_id );
            peer.slabref.send( &self.my_ref, &request_memo.clone() );
            sent += 1;
        }

        sent
    }
        /// Notify interested parties about a newly arrived memoref on this slab
    pub fn dispatch_memoref (&self, memoref : MemoRef){
        //println!("# \t\\ Slab({}).dispatch_memoref({}, {:?}, {:?})", self.id, &memoref.id, &memoref.subject_id, memoref.get_memo_if_resident() );

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
        //println!("Slab({}).handle_memo_from_other_slab({:?})", self.id, memo );

        match memo.body {
            // This Memo is a peering status update for another memo
            MemoBody::SlabPresence{ p: ref presence, r: ref root_index_seed } => {

                match root_index_seed {
                    &MemoRefHead::Subject{..} | &MemoRefHead::Anonymous{..} => {
                        // HACK - this should be done inside the deserialize
                        for memoref in root_index_seed.iter() {
                            memoref.update_peer(origin_slabref, MemoPeeringStatus::Resident);
                        }

                        self.net.apply_root_index_seed( &presence, root_index_seed, &self.my_ref );
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
                for peer in peerlist.iter().filter(|p| p.slabref.0.slab_id != self.id ) {
                    peered_memoref.update_peer( &peer.slabref, peer.status.clone());
                }

                if 0 == peered_memoref.want_peer_count() {
                    self.remove_from_durability_remediation(peered_memoref);
                }
            },
            MemoBody::MemoRequest(ref desired_memo_ids, ref requesting_slabref ) => {

                if requesting_slabref.0.slab_id != self.id {
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
                                    MemoPeerList::new(vec![MemoPeer{
                                        slabref: self.my_ref.clone(),
                                        status: MemoPeeringStatus::NonParticipating
                                    }])
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
                peer_ref.send( &self.my_ref, memoref );
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
        self.counters.read().unwrap().memos_received as u64
    }
    pub fn count_of_memos_reduntantly_received( &self ) -> u64 {
        self.counters.read().unwrap().memos_redundantly_received as u64
    }
    pub fn peer_slab_count (&self) -> usize {
        self.peer_refs.read().unwrap().len() as usize
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
        SlabPresence {
            slab_id: self.id,
            address: origin_slabref.get_return_address(),
            lifetime: SlabAnticipatedLifetime::Unknown
        }
    }
    pub fn slabref_from_presence(&self, presence: &SlabPresence) -> Result<SlabRef,&str> {
            match presence.address {
                TransportAddress::Simulator  => {
                    return Err("Invalid - Cannot create simulator slabref from presence")
                }
                TransportAddress::Local      => {
                    return Err("Invalid - Cannot create local slabref from presence")
                }
                _ => { }
            };


        //let args = TransmitterArgs::Remote( &presence.slab_id, &presence.address );

        Ok(self.put_slabref( presence.slab_id, &vec![presence.clone()] ))
    }



        //             if slab.request_memo(self) > 0 {
        //         channel = slab.memo_wait_channel(self.id);
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
        //             //println!("Slab({}).MemoRef({}).get_memo() received memo: {}", self.owning_slab_id, self.id, memo.id );
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
            .field("slab_id", &self.id)
            .finish()
    }
}