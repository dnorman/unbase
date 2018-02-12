use std::collections::HashMap;
use std::sync::Arc;
use futures::future;
use futures::prelude::*;
use futures::sync::{mpsc,oneshot};
use futures::{Stream,Future};

use slab::storage::StorageInterfaceCore;
use subject::SubjectId;
use network::{Network,Transmitter};
use slab;
use slab::prelude::*;
use slab::counter::SlabCounter;
use memorefhead::MemoRefHead;
use error::*;

struct MemoCarrier{
    memo:      Option<Memo>,
    memoref:   Option<MemoRef>,
    peerstate: Vec<MemoPeerState>,
}

pub struct MemoryCore {
    // * things which will probably be nearly identical across slab types
    /// The Slabref for this slab
    pub slab_id: slab::SlabId,
    net: Network,
    counter: Arc<SlabCounter>,
    // peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    // peering_remediation_queue: Mutex<Vec<MemoRef>>,

    // * Things that should probably be memory resident for most slab types *
    memo_wait_channels: HashMap<MemoId,Vec<oneshot::Sender<Memo>>>,
    subject_subscriptions: HashMap<SubjectId, Vec<mpsc::Sender<MemoRefHead>>>,
    index_subscriptions: Vec<mpsc::Sender<MemoRefHead>>,
    slab_transmitters: HashMap<slab::SlabId,Transmitter>, // TODO: Make this an LRU

    // * Things that would be serialized in most other slab types *
    // Arguably it's simpler to store presence and transmitters togethere here, given that this is a
    // no-serialization slab, However I am intentionally keeping these separate from transmitters
    // for illustrative purpose
    slab_presence_storage: HashMap<slab::SlabId, Vec<SlabPresence>>, 
    memo_storage: HashMap<MemoId,MemoCarrier>,
}

impl MemoryCore {
    pub fn new ( slab_id: slab::SlabId, net: Network, counter: Arc<SlabCounter> ) -> Self {
        MemoryCore{
            slab_id,
            net,
            counter,

            memo_storage:          HashMap::new(),
            slab_presence_storage: HashMap::new(),

            memo_wait_channels:    HashMap::new(),
            subject_subscriptions: HashMap::new(),
            index_subscriptions:   Vec::new(),
            slab_transmitters:    HashMap::new(),
        }
    }
    fn get_slabref(&self) -> SlabRef {
        SlabRef{
            owning_slab_id: self.slab_id,
            slab_id: self.slab_id
        }
    }
    fn get_transmitter(&self, slabref: &SlabRef) -> Result<&Transmitter,Error> {
        use std::collections::hash_map::Entry::*;
        match self.slab_transmitters.entry(slabref.slab_id()) {
            Occupied(t) => {
                Ok(t.get())
            },
            Vacant(t) => {
                //let new_trans = self.net.get_transmitter( &args ).expect("put_slabref net.get_transmitter");
                //let return_address = self.net.get_return_address( &new_presence.address ).expect("return address not found");

                if let Some(presences) = self.slab_presence_storage.get(&slabref.slab_id()) {
                    for presence in presences.iter() {
                        if let Some(transmitter) = presence.get_transmitter( &self.net ){
                            return Ok(t.insert(transmitter));
                        }else{
                            return Err(Error::TransmitError(TransmitError::InvalidTransmitter))
                        }
                    }
                }

                Err(Error::TransmitError(TransmitError::SlabPresenceNotFound))
            }
        }
    }
}

impl StorageInterfaceCore for MemoryCore {
    fn get_memo ( &self, memoref: MemoRef ) -> Box<Future<Item=Option<Memo>, Error=Error>> {
        //println!("# Slab({}).SlabRef({}).send_memo({:?})", self.owning_slab_id, self.slab_id, memoref );

        // QUESTION: is it sensible to interrogate the memoref for the memo itself? I'm starting to doubt it
        // if let Some(memo) = memoref.get_memo_if_resident(){
        //     return Box::new(future::result(Ok(Some(memo))));
        // }

        let maybe_memo = match self.memo_storage.get(&memoref.memo_id()){
            Some(&MemoCarrier{ memo: Some(ref memo), .. }) => Some(memo.clone()),
            _                                              => None
        };

        Box::new(future::result(Ok(maybe_memo)))
    }
    fn send_memo ( &self, slabref: SlabRef, memoref: MemoRef ) -> Box<Future<Item=(), Error=Error>> { //Box<Future<Item=LocalSlabResponse, Error=Error>>  {
        //println!("# Slab({}).SlabRef({}).send_memo({:?})", self.owning_slab_id, self.slab_id, memoref );

        //TODO: accept a list of slabs, and split out the serialization so we can:
        //      1. skip it in cases when we are retrieving from an already-serialized source
        //      2. perform it once when sending to multiple slabs

        // TODO: figure out how to handle multiple transmitters to the same slab. Presumably this will require some thought of transmitter health, latency, and redundancy
        //       we could just spray out to all transmitters for a given slab, but making this a vec introduces other complexity, because we'd have to prune the list rather than just overwriting

        // TODO: update transmitter to return a future?
        if let Some(&MemoCarrier{ memo: Some(ref memo), ref peerstate, .. }) = self.memo_storage.get(&memoref.memo_id()){
            match self.get_transmitter( &slabref ) {
                    Ok(transmitter) => {
                        transmitter.send( memo.clone(), peerstate.clone(), self.get_slabref() )
                    },
                    Err(e) => future::result(Err(e))
                }
                ;
                unimplemented!()
        }else{
            Box::new(future::result(Err(Error::RetrieveError(RetrieveError::NotFound))))
        }
    }
    fn put_memo(&self, memo: Memo, peerstate: Vec<MemoPeerState>, from_slabref: SlabRef ) -> Box<Future<Item=(), Error=Error>>{

    //     let (memoref, had_memoref) = self.assert_memoref(memo.id, memo.subject_id, peerlist.clone(), Some(memo.clone()) );

    //     {
    //         let mut counters = self.counters.write().unwrap();
    //         counters.memos_received += 1;
    //         if had_memoref {
    //             counters.memos_redundantly_received += 1;
    //         }
    //     }
    //     //println!("Slab({}).reconstitute_memo({}) B -> {:?}", self.id, memo_id, memoref );


    //     self.consider_emit_memo(&memoref);

    //     if let Some(ref memo) = memoref.get_memo_if_resident() {

    //         self.check_memo_waiters(memo);
    //         //TODO1 - figure out eventual consistency index update behavior. Think fairly hard about blockchain fan-in / block-tree
    //         // NOTE: this might be a correct place to employ selective hearing. Highest liklihood if the subject is in any of our contexts,
    //         // otherwise 
    //         self.handle_memo_from_other_slab(memo, &memoref, &origin_slabref);
    //         self.do_peering(&memoref, &origin_slabref);

    //     }

    //     self.dispatch_memoref(memoref);

        unimplemented!()
    }
    fn put_slab_presence(&self, presence: SlabPresence ) -> Box<Future<Item=(), Error=Error>> {
        use std::mem;
        use std::collections::hash_map::Entry::*;
        match self.slab_presence_storage.entry(presence.slab_id) {
            Occupied(e) => {
                for p in e.get().iter_mut(){
                    if p == &presence {
                        mem::replace( p, presence ); // Update anticipated liftime
                        break;
                    }
                }
            },
            Vacant(e) => {
                e.insert(vec![presence]);
            }
        };

        // TODO: update transmitter?
        Box::new(future::result(Ok( () )))
    }
    fn get_peerstate (&self, memoref: MemoRef, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerState>, Error=Error>> {
        //println!("MemoRef({}).get_peerlist_for_peer({:?},{:?})", self.id, my_ref, maybe_dest_slab_id);

        if let Some(carrier) = self.memo_storage.get( &memoref.memo_id() ){

            let mut peerstate : Vec<MemoPeerState> =
                if let Some(dest_slabref) = maybe_dest_slabref {
                    // Tell the peer about all other presences except for ones belonging to them
                    // we don't need to tell them they have it. They know, they were there :)
                    carrier.peerstate.iter().filter(|p| p.slabref != dest_slabref).map(|p| p.clone()).collect()
                }else{
                    carrier.peerstate.clone()
                };

            let my_status = match carrier.memo {
                Some(_) => MemoPeerStatus::Resident,
                None    => MemoPeerStatus::Participating,
            };

            peerstate.push(MemoPeerState{
                slabref: self.get_slabref(),
                status: my_status
            });
            Box::new(future::result(Ok(peerstate)))
        }else{
            Box::new(future::result(Ok(vec![])))
        }

    }
}