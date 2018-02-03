use std::thread;
use std::collections::HashMap;
use std::sync::Arc;
use futures::future;
use futures::prelude::*;
use futures::sync::{mpsc,oneshot};
use tokio_core;

use subject::SubjectId;
use network::{Network,Transmitter,TransmitterArgs,TransportAddress};
use slab::prelude::*;
use slab::counter::SlabCounter;
use memorefhead::MemoRefHead;
use error::*;

struct MemoCarrier{
    memo:    Option<Memo>,
    memoref: Option<MemoRef>,
}

pub struct MemoryWorker {
    // * things which will probably be nearly identical across slab types
    pub slab_id: SlabId,
    net: Network,
    counter: Arc<SlabCounter>,
    // peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    // peering_remediation_queue: Mutex<Vec<MemoRef>>,
    //pub my_ref: SlabRef,

    // * Things that should probably be memory resident for most slab types *
    memo_wait_channels: HashMap<MemoId,Vec<oneshot::Sender<Memo>>>,
    subject_subscriptions: HashMap<SubjectId, Vec<mpsc::Sender<MemoRefHead>>>,
    index_subscriptions: Vec<mpsc::Sender<MemoRefHead>>,
    slab_transmitters: HashMap<SlabId,Transmitter>, // TODO: Make this an LRU

    // * Things that would be serialized in most other slab types *
    // Arguably it's simpler to store presence and transmitters togethere here, given that this is a
    // no-serialization slab, However I am intentionally keeping these separate from transmitters
    // for illustrative purpose
    slab_presence_storage: HashMap<SlabId, Vec<SlabPresence>>, 
    memo_storage: HashMap<MemoId,MemoCarrier>,
}


impl MemoryWorker {
    pub fn spawn ( slab_id: SlabId, net: Network, counter: Arc<SlabCounter> ) -> (LocalSlabRequester, thread::JoinHandle<()>) {
        let me = MemoryWorker{
            slab_id,
            net,
            counter,

            memo_storage:          HashMap::new(),
            slab_presence_storage: HashMap::new(),

            memo_wait_channels:    HashMap::new(),
            subject_subscriptions: HashMap::new(),
            index_subscriptions:   Vec::new(),
            slab_transmitters:    HashMap::new(),
        };

        let (tx,rx) = mpsc::unbounded::<(LocalSlabRequest,oneshot::Sender<Result<LocalSlabResponse,Error>>)>();

        let worker_thread = thread::spawn(move || {
            let mut core = tokio_core::reactor::Core::new().unwrap();
            let server = rx.for_each(|(request, resp_channel)| {
                me.dispatch_request(request,resp_channel)
            });

            core.run(server).unwrap();
        });

        (tx,worker_thread)
    }
    fn dispatch_request(&self,request: LocalSlabRequest, responder: oneshot::Sender<Result<LocalSlabResponse,Error>>) -> Box<Future<Item=(), Error=()>> {
        use slab::common_structs::LocalSlabRequest::*;
        let f = match request {
            SendMemo {slab_id, memoref}    => self.send_memo(slab_id, memoref),
            PutSlabPresence { presence }   => self.put_slab_presence(presence),
        }.then(|response| {
            responder.send(response)
        }).then(|_| {
            Ok(())
        });

        Box::new(f)
    }
    pub fn put_slab_presence(&self, presence: SlabPresence ) -> Box<Future<Item=LocalSlabResponse, Error=Error>> {
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
        Box::new(future::result(Ok(LocalSlabResponse::PutSlabPresence(()))))
    }
    pub fn send_memo ( &self, slab_id: SlabId, memoref: MemoRef ) -> Box<Future<Item=LocalSlabResponse, Error=Error>>  {
        //println!("# Slab({}).SlabRef({}).send_memo({:?})", self.owning_slab_id, self.slab_id, memoref );


        use std::collections::hash_map::Entry::*;
        match self.slab_transmitters.entry(slab_id) {
            Occupied(t) => {
                let transmitter = t.get();
                transmitter.send( self.slab_id, memoref.clone() ).map(|_| LocalSlabResponse::SendMemo(()))
            },
            Vacant(t) => {
                //let new_trans = self.net.get_transmitter( &args ).expect("put_slabref net.get_transmitter");
                //let return_address = self.net.get_return_address( &new_presence.address ).expect("return address not found");

                match self.slab_presence_storage.entry(slab_id){
                    Occupied(p) => {
                        let presences = p.get();
                        for presence in presences {
                            if let Some(transmitter) = presence.get_transmitter( &self.net ){
                                t.insert(transmitter);
                                break;
                            }
                        }
                    },
                    Vacant(_p) => {
                        Box::new(future::result(Err(Error::TransmitError(TransmitError::SlabPresenceNotFound))))
                    }
                }
            }
        }
    }
    pub fn get_memo ( &self, memo_id: MemoId ) -> Box<Future<Item=LocalSlabResponse, Error=Error>>  {
        //println!("# Slab({}).SlabRef({}).send_memo({:?})", self.owning_slab_id, self.slab_id, memoref );

        let maybe_memo = match self.memo_storage.get(&memo_id){
            Some(&MemoCarrier{ memo: Some(ref memo), .. }) => Some(memo.clone()),
            _                                              => None
        };

        Box::new(future::result(Ok(LocalSlabResponse::GetMemo(maybe_memo))))
    }
}