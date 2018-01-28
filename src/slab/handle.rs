use std::sync::Arc;
use core::ops::Deref;
use futures::*;
use futures::sync::{mpsc,oneshot};
use std::fmt;

use network;
use subject::SubjectId;
use memorefhead::MemoRefHead;
use slab::prelude::*;
use error::*;

// handle speaks with the slab inner
// slab inner is held in memory by the slab itself?

#[derive(Clone)]
pub struct SlabHandle(Arc<SlabHandleInner>);

impl Deref for SlabHandle {
    type Target = SlabHandleInner;
    fn deref(&self) -> &SlabHandleInner {
        &*self.0
    }
}

struct SlabHandleInner {
    pub id: SlabId,
    pub tx: mpsc::UnboundedSender<SlabRequest>,
    pub my_ref: SlabRef,
}

impl SlabHandle {
    pub fn initialize (slab_id: SlabId) -> (SlabHandle, mpsc::UnboundedReceiver<(SlabRequest,oneshot::Sender<SlabResponse>)>) {
        let (tx,rx) = mpsc::unbounded();
        let handle = SlabHandle(Arc::new(SlabHandleInner{
            id: slab_id,
            tx: tx,
        }));

        (handle,rx)
    }
}

// QUESTION - how can SlabHandle be made to forego the channel communication with the slab in cases where the whole app actually wants to run in one eventloop?
//            Intuitively this seems like it could be done in a relatively straghtforward manner once the channel/futures conversion is done,
//            but I've not yet thought much about it directly - It's a bit outside of my present comprehension.

impl SlabHandleInner {
    fn call (&self, request: SlabRequest ) -> Box<Future<Item=SlabResponse, Error=Error>> {
        let (p, c) = oneshot::channel::<SlabResponse>();
        unimplemented!()
    }
    pub fn register_local_slabref(&self, peer_slab: &SlabHandle) {

        //let args = TransmitterArgs::Local(&peer_slab);
        let presence = SlabPresence{
            slab_id: peer_slab.id,
            address: network::transport::TransportAddress::Local,
            lifetime: SlabAnticipatedLifetime::Unknown
        };

        self.put_slabref(peer_slab.id, &vec![presence])
    }
    pub fn is_live (&self) -> bool {
        unimplemented!()
    }
    pub fn receive_memo_with_peerlist(&self, memo: Memo, peerlist: MemoPeerList, from_slabref: SlabRef ) {
        self.call(SlabRequest::ReceiveMemoWithPeerList{ memo, peerlist, from_slabref } ).wait()
    }
    pub fn put_slabref(&self, slab_id: SlabId, presence: &[SlabPresence] ) -> SlabRef {
        self.call(SlabRequest::AssertSlabRef{ slab_id, presence } ).wait()?
    }
    pub fn remotize_memo_ids( &self, memo_ids: &[MemoId] ) -> Box<Future<Item=(), Error=Error>>  { 
            self.call(SlabRequest::RemotizeMemoIds{ memo_ids } ).wait()?
    }
    
    pub fn get_memoref {
        
    }
    pub fn get_memo (&self, memo_id: MemoId ) -> Box<Future<Item=Memo, Error=Error>> {
        if let SlabResponse::GetMemo(memo) = self.call(SlabRequest::GetMemo{ memo_id } ) {

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
    }

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
    //         match self.call(SlabRequest::RemotizeMemoIds{ memo_ids } ).wait() {
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


impl fmt::Debug for SlabHandle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SlabHandle")
            .field("slab_id", &self.id)
            .finish()
    }
}