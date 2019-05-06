use std::rc::Rc;
use std::cell::RefCell;

//use futures::future;
use futures::prelude::*;
use futures::sync::mpsc;
use std::fmt;

use crate::network;
use crate::slab;
use crate::slab::store::StoreHandle;
use crate::slab::prelude::*;
use crate::slab::counter::SlabCounter;
use crate::error::*;
use crate::subject::{SubjectId,SubjectType};
use crate::memorefhead::MemoRefHead;

use super::store::{LocalSlabRequest,LocalSlabResponse};

impl LocalSlabHandle {
    pub fn new (slabref: SlabRef, counter: Rc<SlabCounter>, store: StoreHandle) -> LocalSlabHandle {

        LocalSlabHandle{
            slab_id: slabref.slab_id(),
            slabref,
            counter,
            store,
        }
    }

    pub fn get_memo (&self, memoref: MemoRef, allow_remote: bool ) -> Box<Future<Item=Memo, Error=Error>>{

        self.store.request_deadline(LocalSlabRequest::GetMemo{ memoref, allow_remote }, 1000 ).and_then(|r| {
            if let LocalSlabResponse::GetMemo(memo) = r {
               Ok(memo)
            }else{
                panic!("Invalid return type");
            }
        })
    }
    pub fn put_memo(&self, memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef ) -> Box<Future<Item=MemoRef, Error=Error>>{
        self.store.request(LocalSlabRequest::PutMemo{ memo, peerset, from_slabref } ).and_then(|r| {
            if let LocalSlabResponse::PutMemo(memoref) = r {
                return Ok(memoref);
            }else{
                panic!("Invalid return type");
            }
        })
    }
    pub fn put_slab_presence(&mut self, presence: SlabPresence ) -> SlabRef {
        self.store.borrow_mut().put_slab_presence(presence.clone()).wait().unwrap();

        SlabRef{
            owning_slab_id: self.slab_id,
            slab_id: presence.slab_id
        }
    }

    pub fn put_slab_presence ( &self, presence: SlabPresence ) -> Box<Future<Item=(), Error=Error>>{
        Box::new( self.call(LocalSlabRequest::PutSlabPresence{ presence } ).and_then(|r| {
            if let LocalSlabResponse::SendMemo(_) = r {
                return Ok(())
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    pub fn put_memoref( &self, memo_id: MemoId, subject_id: SubjectId, peerset: MemoPeerSet) -> Box<Future<Item=MemoRef, Error=Error>> {
        // TODO: Implement
        unimplemented!()
    }
    pub fn send_memos ( &self, to_slabrefs: &[SlabRef], memorefs: &[MemoRef] ) -> Box<Future<Item=(), Error=Error>>{
        Box::new( self.call(LocalSlabRequest::SendMemo{ to_slabrefs: to_slabrefs.to_vec(), memorefs: memorefs.to_vec() } ).and_then(|r| {
            if let LocalSlabResponse::SendMemo(_) = r {
                return Ok(())
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    // LEFT OFF HERE
    pub fn get_slab_presence(&mut self, slabrefs: Vec<SlabRef>) -> Result<Vec<SlabPresence>, Error> {
        self.store.borrow_mut().get_slab_presence(slabrefs).wait()
    }
    pub fn get_slab_presence ( &self, slabrefs: Vec<SlabRef>) -> Box<Future<Item=Vec<SlabPresence>, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetSlabPresence{ slabrefs } ).and_then(|r| {
            if let LocalSlabResponse::GetSlabPresence(presences) = r {
                return Ok(presences)
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    pub fn get_peerset(&mut self, memorefs: Vec<MemoRef>, maybe_dest_slabref: Option<SlabRef>) -> Result<Vec<MemoPeerSet>, Error> {
        self.store.borrow_mut().get_peerset(memorefs, maybe_dest_slabref).wait()
    }
    pub fn get_peerset ( &self, memorefs: Vec<MemoRef>, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerSet>, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetPeerSet{ memorefs, maybe_dest_slabref } ).and_then(|r| {
            if let LocalSlabResponse::GetPeerSet(peersets) = r {
                return Ok(peersets)
            }else{
                panic!("Invalid return type");
            }
        }))
    }


    pub fn slab_id(&self) -> slab::SlabId {
        self.slab_id.clone()
    }
    pub fn register_local_slabref(&mut self, peer_slab: &LocalSlabHandle) {

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
            slab_id
        }
    }
    pub fn is_live (&self) -> bool {
        unimplemented!()
    }
    pub fn receive_memo_with_peerlist(&self, _memo: Memo, _peerlist: Vec<MemoPeerState>, _from_slabref: SlabRef ) {
        unimplemented!();
        // self.call(LocalSlabRequest::ReceiveMemoWithPeerList{ memo, peerlist, from_slabref } ).wait()
    }

    pub fn assert_memoref( &self, _memo_id: MemoId, _subject_id: SubjectId, _peerset: MemoPeerSet, _maybe_memo: Option<Memo>) -> (MemoRef, bool){
        unimplemented!()
    }
    pub fn remotize_memo_ids( &self, _memo_ids: &[MemoId] ) -> Box<Future<Item=(), Error=Error>>  {
        unimplemented!();
    }
    pub (crate) fn observe_subject (&self, _subject_id: SubjectId ) -> Box<Stream<Item = MemoRefHead, Error = Error>> {

        // let (tx, rx) = mpsc::channel::<MemoRefHead>(1);
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
    pub (crate) fn observe_index (&self) -> mpsc::Receiver<MemoRefHead> {
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
    pub fn new_memo (&self, subject_id: SubjectId, parents: MemoRefHead, body: MemoBody) -> Box<Future<Item=MemoRef,Error=Error>> {
        let memo_id = (self.slab_id as u64).rotate_left(32) | self.counter.next_memo_id() as u64;

        //println!("# Slab({}).new_memo(id: {},subject_id: {:?}, parents: {:?}, body: {:?})", self.slab_id, memo_id, subject_id, parents.memo_ids(), body );

        let memo = Memo {
            id:    memo_id,
            subject_id,
            parents,
            body,

            #[cfg(debug_assertions)]
            owning_slabref: self.slabref.clone(),
        };

        self.put_memo( memo, MemoPeerSet::empty(), self.slabref )

    }
    pub fn remotize_memoref( &self, memoref: &MemoRef ) -> Result<(),Error> {

        #[cfg(debug_assertions)]
        assert_eq!(memoref.owning_slab_id, self.slab_id);

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
        //     peer.slabref.send( &self.slabref, &peering_memoref );
        // }

        // Ok(())
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
    pub fn new_memo_basic (&self, subject_id: SubjectId, parents: MemoRefHead, body: MemoBody) -> Box<Future<Item=MemoRef, Error=Error>> {
        self.new_memo(subject_id, parents, body)
    }
    pub fn new_memo_basic_noparent (&self, subject_id: SubjectId, body: MemoBody) -> Box<Future<Item=MemoRef, Error=Error>> {
        self.new_memo(subject_id, MemoRefHead::Null, body)
    }

}


impl fmt::Debug for LocalSlabHandle{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("LocalSlabHandle")
            .field("slab_id", &self.slab_id)
            .finish()
    }
}