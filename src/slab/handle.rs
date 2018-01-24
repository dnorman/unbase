use std::sync::Arc;
use core::ops::Deref;
use futures::*;
use futures::sync::{mpsc,oneshot};

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
    pub fn is_live (&self) -> bool {
        unimplemented!()
    }
    pub fn receive_memo_with_peerlist(&self, memo: Memo, peerlist: MemoPeerList, from_slabref: SlabRef ) {
        self.call(SlabRequest::ReceiveMemoWithPeerList{ memo, peerlist, from_slabref } ).wait()
    }
    pub fn assert_slabref(&self, slab_id: SlabId, presence: &[SlabPresence] ) -> SlabRef {
        self.call(SlabRequest::AssertSlabRef{ slab_id, presence } ).wait()?
    }
    pub fn remotize_memo_ids_wait( &self, memo_ids: &[MemoId], ms: u64 ) -> Result<(),Error> {
        use std::time::{Instant,Duration};
        let start = Instant::now();
        let wait = Duration::from_millis(ms);
        use std::thread;

        loop {
            if start.elapsed() > wait{
                return Err(Error::StorageOpDeclined(StorageOpDeclined::InsufficientPeering))
            }

            #[allow(unreachable_patterns)]
            match self.call(SlabRequest::RemotizeMemoIds{ memo_ids } ).wait() {
                Ok(_) => {
                    return Ok(())
                },
                Err(Error::StorageOpDeclined(StorageOpDeclined::InsufficientPeering)) => {}
                Err(e)                                      => return Err(e)
            }

            thread::sleep(Duration::from_millis(50));
        }
    }
}


