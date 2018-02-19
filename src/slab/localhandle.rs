use std::sync::Arc;
//use futures::future;
use futures::prelude::*;
use futures::sync::mpsc;
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

        LocalSlabHandle{
            slab_id: slabref.slab_id(),
            slabref,
            counter,
            storage,
        }
    }

    pub fn get_memo (&self, _memoref: MemoRef, _allow_remote: bool ) -> Box<Future<Item=Memo, Error=Error>> {
//        use slab::storage::StorageMemoRetrieval::*;
//        self.storage.get_memo(memoref,allow_remote).and_then(|r|{
//            match r {
//                Found(memo)     => Box::new(future::result(Ok(memo))),
//                Remote(peerset) => {
//                }
//            }
//        });

        unimplemented!()
        //                 let timeout = tokio_core::reactor::Timeout::new(Duration::from_millis(170), &handle).unwrap();

        // let work = request.select2(timeout).then(|res| match res {
        //     Ok(Either::A((got, _timeout))) => Ok(got),
        //     Ok(Either::B((_timeout_error, _get))) => {
        //         Err(hyper::Error::Io(io::Error::new(
        //             io::ErrorKind::TimedOut,
        //             "Client timed out while connecting",
        //         )))
        //     }
        //     Err(Either::A((get_error, _timeout))) => Err(get_error),
        //     Err(Either::B((timeout_error, _get))) => Err(From::from(timeout_error)),
        // });
    }
    pub fn put_memo(&self, memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef ) -> Box<Future<Item=MemoRef, Error=Error>> {
        self.storage.put_memo(memo, peerset, from_slabref)
    }
    pub fn put_slab_presence(&self, presence: SlabPresence ) -> SlabRef {
        self.storage.put_slab_presence(presence).wait().unwrap();

        SlabRef{
            owning_slab_id: self.slab_id,
            slab_id: presence.slab_id
        }
    }
    pub fn get_peerset(&self, memoref: MemoRef, maybe_dest_slabref: Option<SlabRef>) -> Result<MemoPeerSet, Error> {
        self.storage.get_peerset(memoref, maybe_dest_slabref).wait()
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
    pub (crate) fn observe_subject (&self, _subject_id: SubjectId, _tx: mpsc::Sender<MemoRefHead> ) {

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
    pub (crate) fn observe_index (&self, _tx: mpsc::Sender<MemoRefHead> ) {
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
    pub fn new_memo ( &self, subject_id: SubjectId, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        let memo_id = (self.slab_id as u64).rotate_left(32) | self.counter.next_memo_id() as u64;

        //println!("# Slab({}).new_memo(id: {},subject_id: {:?}, parents: {:?}, body: {:?})", self.slab_id, memo_id, subject_id, parents.memo_ids(), body );

        let _memo = Memo {
            id:    memo_id,
            owning_slabref: self.slabref.clone(),
            subject_id,
            parents,
            body
        };

        // TODO1 figure out if put_memo should give back a remoref. Probably Yes??
        unimplemented!();
        //let (memoref, _had_memoref) = self.put_memo( memo, vec![], self.slabref ).wait();

        // TODO1 figure out where and how this should be called
        //self.consider_emit_memo(&memoref);

        //memoref
    }
    pub fn remotize_memoref( &self, memoref: &MemoRef ) -> Result<(),Error> {
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
    pub fn new_memo_basic (&self, subject_id: SubjectId, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        self.new_memo(subject_id, parents, body)
    }
    pub fn new_memo_basic_noparent (&self, subject_id: SubjectId, body: MemoBody) -> MemoRef {
        self.new_memo(subject_id, MemoRefHead::Null, body)
    }

}


impl fmt::Debug for LocalSlabHandle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("LocalSlabHandle")
            .field("slab_id", &self.slab_id)
            .finish()
    }
}