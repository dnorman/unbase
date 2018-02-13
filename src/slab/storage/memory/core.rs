use std::collections::HashMap;
use std::sync::Arc;
use futures::{future, Future, prelude::*, sync::mpsc};

use slab::storage::{StorageCore, StorageCoreInterface};
use network::{Network,Transmitter};
use slab;
use slab::prelude::*;
use slab::counter::SlabCounter;
use error::*;
use slab::dispatcher::MemoDispatch;

struct MemoCarrier{
    memoref:  MemoRef,
    memo:     Option<Memo>,
    peerset:  MemoPeerSet,
}

pub struct MemoryCore {
    // * things which will probably be nearly identical across slab types
    /// The Slabref for this slab
    pub slab_id: slab::SlabId,
    net: Network,
    counter: Arc<SlabCounter>,
    // peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    // peering_remediation_queue: Mutex<Vec<MemoRef>>,

    dispatcher_tx: mpsc::UnboundedSender<MemoDispatch>,
    slab_transmitters: HashMap<slab::SlabId,Transmitter>, // TODO: Make this an LRU

    // * Things that would be serialized in most other slab types *
    // Arguably it's simpler to store presence and transmitters togethere here, given that this is a
    // no-serialization slab, However I am intentionally keeping these separate from transmitters
    // for illustrative purpose
    slab_presence_storage: HashMap<slab::SlabId, Vec<SlabPresence>>, 
    memo_storage: HashMap<MemoId,MemoCarrier>,
}

impl MemoryCore {
    pub fn new ( slab_id: slab::SlabId, net: Network, counter: Arc<SlabCounter>, dispatcher_tx: mpsc::UnboundedSender<MemoDispatch> ) -> Self {
        MemoryCore{
            slab_id,
            net,
            counter,

            memo_storage:          HashMap::new(),
            slab_presence_storage: HashMap::new(),

            // I think we want to have the transmitters locally accessible
            // so we can send stuff directly without deserializing
            slab_transmitters:    HashMap::new(),

            dispatcher_tx,
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
impl StorageCore for MemoryCore {
    fn slab_id (&self) -> slab::SlabId {
        self.slab_id.clone()
    }
}

impl StorageCoreInterface for MemoryCore {
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
        if let Some(&MemoCarrier{ memo: Some(ref memo), ref peerset, .. }) = self.memo_storage.get(&memoref.memo_id()){
            match self.get_transmitter( &slabref ) {
                    Ok(transmitter) => {
                        transmitter.send( memo.clone(), peerset.clone(), self.get_slabref() )
                    },
                    Err(e) => Box::new(future::result(Err(e)))
                }
                ;
                unimplemented!()
        }else{
            Box::new(future::result(Err(Error::RetrieveError(RetrieveError::NotFound))))
        }
    }
    fn put_memo(&self, memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef ) -> Box<Future<Item=MemoRef, Error=Error>>{

        self.counter.increment_memos_received();
        use std::collections::hash_map::Entry::*;
        let memoref = match self.memo_storage.entry(memo.id) {
            Vacant(e)   => {
                let mr = MemoRef::new(self, memo.id.clone(), memo.subject_id.clone());
                e.insert(MemoCarrier{
                    memoref: mr.clone(),
                    memo: Some(memo.clone()),
                    peerset
                });
                mr
            }
            Occupied(e) => {
                let mut carrier = e.get();
                if carrier.memo.is_some(){
                    self.counter.increment_memos_redundantly_received()
                }
                carrier.memo = Some(memo.clone());
                carrier.peerset.apply_peerset( &peerset );
                carrier.memoref.clone()
            }
        };

        self.dispatcher_tx.unbounded_send(MemoDispatch{memo, memoref: memoref.clone(), from_slabref});

        Box::new(future::result(Ok(memoref)))
    }
        // fn assert_memoref( &self, memo_id: MemoId, subject_id: SubjectId, peerlist: MemoPeerList, maybe_memo: Option<Memo>) -> (MemoRef, bool){


    //     let had_memoref;
    //     let memoref = match self.memorefs_by_id.entry(memo_id) {
    //         Entry::Vacant(o)   => {
    //             let mr = MemoRef(Arc::new(
    //                 MemoRefInner {
    //                     id: memo_id,
    //                     owning_slab_id: self.id,
    //                     subject_id: subject_id,
    //                     peerlist: RwLock::new(peerlist),
    //                     ptr:      RwLock::new(match memo {
    //                         Some(m) => {
    //                             assert!(self.id == m.owning_slab_id);
    //                             MemoRefPtr::Resident(m)
    //                         }
    //                         None    => MemoRefPtr::Remote
    //                     })
    //                 }
    //             ));

    //             had_memoref = false;
    //             o.insert( mr ).clone()// TODO: figure out how to prolong the borrow here & avoid clone
    //         }
    //         Entry::Occupied(o) => {
    //             let mr = o.get();
    //             had_memoref = true;
    //             if let Some(m) = memo {

    //                 let mut ptr = mr.ptr.write().unwrap();
    //                 if let MemoRefPtr::Remote = *ptr {
    //                     *ptr = MemoRefPtr::Resident(m)
    //                 }
    //             }
    //             mr.apply_peers( &peerlist );
    //             mr.clone()
    //         }
    //     };

    //     (memoref, had_memoref)
    // }
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
    fn get_peerset (&self, memoref: MemoRef, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=MemoPeerSet, Error=Error>> {
        //println!("MemoRef({}).get_peerlist_for_peer({:?},{:?})", self.id, my_ref, maybe_dest_slab_id);

        if let Some(carrier) = self.memo_storage.get( &memoref.memo_id() ){

            let mut peerset : MemoPeerSet =
                if let Some(dest_slabref) = maybe_dest_slabref {
                    // Tell the peer about all other presences except for ones belonging to them
                    // we don't need to tell them they have it. They know, they were there :)
                    carrier.peerset.for_slabref(&dest_slabref)
                }else{
                    carrier.peerset.clone()
                };

            let my_status = match carrier.memo {
                Some(_) => MemoPeerStatus::Resident,
                None    => MemoPeerStatus::Participating,
            };

            peerset.apply_peerstate(MemoPeerState{
                slabref: self.get_slabref(),
                status: my_status
            });
            Box::new(future::result(Ok(peerset)))
        }else{
            Box::new(future::result(Ok(MemoPeerSet::new(vec![]))))
        }

    }
}