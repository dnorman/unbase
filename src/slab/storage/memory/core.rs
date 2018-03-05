use std::collections::HashMap;
use std::rc::Rc;
use futures::{self, future, Future, Sink, unsync::{mpsc,oneshot}}; //prelude::*,

use network::{Network,Transmitter};
use buffer::NetworkBuffer;
use slab::{self, prelude::*, counter::SlabCounter, storage::{StorageCore, StorageCoreInterface}};
use subject::SubjectId;
use memorefhead::MemoRefHead;
use error::*;
use slab::dispatcher::Dispatch;
use futures::Stream;


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
    counter: Rc<SlabCounter>,
    // peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    // peering_remediation_queue: Mutex<Vec<MemoRef>>,

    dispatcher_tx: mpsc::Sender<Dispatch>,
    slab_transmitters: HashMap<slab::SlabId,Transmitter>, // TODO: Make this an LRU

    // * Things that would be serialized in most other slab types *
    // Arguably it's simpler to store presence and transmitters togethere here, given that this is a
    // no-serialization slab, However I am intentionally keeping these separate from transmitters
    // for illustrative purpose
    slab_presence_storage: HashMap<slab::SlabId, SlabPresence>,
    memo_storage: HashMap<MemoId,MemoCarrier>,
}

impl MemoryCore {
    pub fn new ( slab_id: slab::SlabId, net: Network, counter: Rc<SlabCounter>, dispatcher_tx: mpsc::Sender<Dispatch> ) -> Self {
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
    fn get_transmitter(&mut self, slabref: &SlabRef) -> Result<&Transmitter,Error> {
        use std::collections::hash_map::Entry::*;
        match self.slab_transmitters.entry(slabref.slab_id()) {
            Occupied(_t) => {
                //Ok((*t.get()).clone()) // TODO - figure out if there's a way to do this with a guard
                unimplemented!()
            },
            Vacant(t) => {
                //let new_trans = self.net.get_transmitter( &args ).expect("put_slabref net.get_transmitter");
                //let return_address = self.net.get_return_address( &new_presence.address ).expect("return address not found");

                if let Some(presence) = self.slab_presence_storage.get(&slabref.slab_id()) {
                    if let Some(transmitter) = presence.get_transmitter( &self.net ){
                        return Ok(t.insert(transmitter));
                    }else{
                        return Err(Error::TransmitError(TransmitError::InvalidTransmitter))
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
    fn get_memo ( &mut self, memoref: MemoRef, allow_remote: bool ) -> Box<Future<Item=Memo, Error=Error>> {
        //println!("# Slab({}).SlabRef({}).send_memo({:?})", self.owning_slab_id, self.slab_id, memoref );

        let request_peers: Vec<SlabRef>;

        match self.memo_storage.get(&memoref.memo_id()) {
            Some(&MemoCarrier{ memo: Some(ref memo), ..}) => {
                return Box::new(future::result(Ok(memo.clone())))
            },
            Some(&MemoCarrier{ ref peerset, ..  }) => {
                if !allow_remote {
                    return Box::new(future::result::<Memo,_>(Err(Error::RetrieveError(RetrieveError::NotFoundLocally))))
                }

                // Send the request
                request_peers = peerset.list.iter().filter(|ps| ps.status == MemoPeerStatus::Resident)
                    .take(5).map(|ps| ps.slabref.clone() ).collect();

                if request_peers.is_empty() {
                    return Box::new(future::result(Err(Error::RetrieveError(RetrieveError::InsufficientPeering))))
                    // TODO: should probably undergo a more agressive search process – querying the index for instance
                }
            },
            None => {
                // IN THEORY we shouldn't get here if there are outstanding memorefs
                // In actuality it will be fairly likely to happen, and will require a remediation strategy
                return Box::new(future::result::<Memo, Error>(Err(Error::RetrieveError(RetrieveError::NotFound))));
            }
        }

        let (tx, rx) = oneshot::channel::<Result<Memo, Error>>();

        // Listen for the returned memo - QUESTION: what ordering guarantees does this channel offer? Could the GotMemo potentially beat the WaitForMemo?
        self.dispatcher_tx.send(Dispatch::WaitForMemo { memoref: memoref.clone(), tx });

        let mut return_presences: Vec<SlabPresence> = request_peers.iter().map(|p| p.return_presence()).collect();
        return_presences.sort();
        return_presences.dedup();

        let request_memo_id = (self.slab_id as u64).rotate_left(32) | self.counter.next_memo_id() as u64;

        // directly Construct and send the request memo. We probably don't need to store it,
        // and we definitely don't need to store it before sending it

        let request_memo = Memo {
            id: request_memo_id,
            subject_id: SubjectId::anonymous(),
            parents: MemoRefHead::Null,
            // TODO: Make MemoRequest use return_presences?
            body: MemoBody::MemoRequest(vec![memoref.clone()], vec![self.get_slabref()]),

            #[cfg(debug_assertions)]
            owning_slabref: self.get_slabref(),
        };

        let memoref =  MemoRef::new(&self.slab_id, request_memo.id.clone(), request_memo.subject_id.clone());

        let peerset = MemoPeerSet::empty();
        let mut netbuf = NetworkBuffer::new( self.get_slabref() );

        netbuf.add_memoref_peerset_and_memo(&memoref, &peerset, &request_memo);
        // We can cheat and avoid populating slab presences / memo peersets because we know there aren't any
        // This may change in the future, but for now it's true
        debug_assert_eq!(netbuf.is_fully_populated(), true);

        let mut sends : Vec<_> = Vec::new();
        for request_peer in request_peers {
            if let Ok(transmitter) = self.get_transmitter(&request_peer) {
                sends.push( transmitter.send(netbuf.clone() ) );
            }
        }

        if sends.is_empty() {
            return Box::new(future::result(Err(Error::RetrieveError(RetrieveError::InsufficientPresence))))
        }

        use futures::sync::oneshot::Canceled;
        Box::new(futures::collect(sends).and_then( |_| {
            rx.then(| response : Result<Result<Memo,Error>,Canceled> | {
                match response {
                    Err(_) => Err(Error::RetrieveError(RetrieveError::SlabError)), // oneshot Error=Canceled
                    Ok(result) => result
                }
            })
        }))

        // TODO: Add timeout and retries
    }
    fn put_memo( &mut self, memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef ) -> Box<Future<Item=MemoRef, Error=Error>>{

        self.counter.increment_memos_received();
        use std::collections::hash_map::Entry::*;
        let memoref = match self.memo_storage.entry(memo.id) {
            Vacant(e)   => {
                let mr = MemoRef::new(&self.slab_id, memo.id.clone(), memo.subject_id.clone());
                e.insert(MemoCarrier{
                    memoref: mr.clone(),
                    memo: Some(memo.clone()),
                    peerset
                });
                mr
            }
            Occupied(mut e) => {
                let mut carrier = e.get_mut();
                if carrier.memo.is_some(){
                    self.counter.increment_memos_redundantly_received()
                }
                carrier.memo = Some(memo.clone());
                carrier.peerset.apply_peerset( peerset );
                carrier.memoref.clone()
            }
        };

        self.dispatcher_tx.send(Dispatch::GotMemo{memo, memoref: memoref.clone(), from_slabref});

        Box::new(future::result(Ok(memoref)))
    }
    fn put_memoref( &mut self, memo_id: MemoId, subject_id: SubjectId, peerset: MemoPeerSet) -> Box<Future<Item=MemoRef, Error=Error>> {
        // TODO:
        unimplemented!()
    }
    fn send_memos ( &mut self, slabrefs: &[SlabRef], memorefs: &[MemoRef]) -> Box<Future<Item=(), Error=Error>> {
        //println!("# Slab({}).SlabRef({}).send_memo({:?})", self.owning_slab_id, self.slab_id, memoref );

        //TODO: accept a list of slabs, and split out the serialization so we can:
        //      1. skip it in cases when we are retrieving from an already-serialized source
        //      2. perform it once when sending to multiple slabs

        // TODO: figure out how to handle multiple transmitters to the same slab. Presumably this will require some thought of transmitter health, latency, and redundancy
        //       we could just spray out to all transmitters for a given slab, but making this a vec introduces other complexity, because we'd have to prune the list rather than just overwriting

        // TODO: update transmitter to return a future?


        let mut netbuf = NetworkBuffer::new( self.get_slabref() );

        for memoref in memorefs {
            if let Some(&MemoCarrier{ memoref: ref memoref, memo: Some(ref memo), ref peerset, .. }) = self.memo_storage.get(&memoref.memo_id()) {
                netbuf.add_memoref_peerset_and_memo(memoref, peerset, memo)
            }
        }

        netbuf.populate_memopeersets(|memoref| {
            if let Some(&MemoCarrier{ ref peerset, .. }) = self.memo_storage.get( &memoref.memo_id() ) {
                peerset.clone()
            }else{
                MemoPeerSet::empty()
            }
        });
        netbuf.populate_slabpresences(|slabref| {
            // TODO: Implement this _and_ populate_slabpresences
            unimplemented!()
            // self.slab_presence_storage.get(*slabref.slab_id).clone()
        });

        debug_assert_eq!(netbuf.is_fully_populated(), true);

        let my_slabref = self.get_slabref();

        let mut sends : Vec<_> = Vec::new();
        for request_peer in slabrefs {
            if let Ok(transmitter) = self.get_transmitter(&request_peer) {
                sends.push( transmitter.send(netbuf.clone() ) );
            }
        }

        if sends.is_empty() {
            return Box::new(future::result(Err(Error::RetrieveError(RetrieveError::InsufficientPresence))))
        }

        Box::new(futures::collect(sends).map( |_| () ) )
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
    fn get_slab_presence( &mut self, slabrefs: Vec<SlabRef> ) -> Box<Future<Item=Vec<SlabPresence>, Error=Error>> {

        let mut out = Vec::with_capacity(slabrefs.len());

        for slabref in slabrefs {
            match self.slab_presence_storage.get(&slabref.slab_id) {
                Some(presence) => {
                    unimplemented!()
                    // TODO: Change get_slab_presence interface to return an Option<SlabPresence>.
                    // out.push(Some(presence));
                }
                None => {
                    unimplemented!()
                    // out.push(None)
                }
            };
        }

        // TODO: update transmitter?
        Box::new(future::result(Ok( out )))
    }
    fn put_slab_presence( &mut self, presence: SlabPresence ) -> Box<Future<Item=(), Error=Error>> {
        use std::collections::hash_map::Entry::*;
        match self.slab_presence_storage.entry(presence.slab_id) {
            Occupied(mut e) => {
                e.insert(presence); // TODO: apply new presence to previous one
            },
            Vacant(e) => {
                e.insert(presence);
            }
        };

        // TODO: update transmitter?
        Box::new(future::result(Ok( () )))
    }
    fn get_peerset ( &mut self, memorefs: Vec<MemoRef>, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerSet>, Error=Error>> {
        //println!("MemoRef({}).get_peerlist_for_peer({:?},{:?})", self.id, my_ref, maybe_dest_slab_id);

        let mut peersets = Vec::new();
        for memoref in memorefs {
            if let Some(carrier) = self.memo_storage.get(&memoref.memo_id()) {
                let mut peerset: MemoPeerSet =
                    if let Some(ref dest_slabref) = maybe_dest_slabref {
                        // Tell the peer about all other presences except for ones belonging to them
                        // we don't need to tell them they have it. They know, they were there :)
                        carrier.peerset.for_slabref(dest_slabref)
                    } else {
                        carrier.peerset.clone()
                    };

                let my_status = match carrier.memo {
                    Some(_) => MemoPeerStatus::Resident,
                    None => MemoPeerStatus::Participating,
                };

                peerset.apply_peerstate(MemoPeerState {
                    slabref: self.get_slabref(),
                    status: my_status
                });
                peersets.push(peerset);
            } else {
                peersets.push( MemoPeerSet::empty() )
            }
        }


        Box::new(future::result(Ok(peersets)))

    }
}