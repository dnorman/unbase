use std::sync::Arc;
use core::ops::Deref;
use futures::sync::oneshot;
use futures::*;

use subject::SubjectId;
use memorefhead::MemoRefHead;
use slab::prelude::*;
use error::*;

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
    pub requests: 
}

impl SlabHandleInner {
    fn call (&self, request: SlabRequest ) -> Box<Future<Item=SlabResponse, Error=Error>> {

        let (p, c) = oneshot::channel::<i32>();

        c
    }
   pub fn is_live (&self) -> bool {
       unimplemented!()
   }
   pub fn receive_memo_with_peerlist(&self, memo: Memo, peerlist: MemoPeerList, from_slabref: SlabRef ) {
       self.call(SlabRequest::ReceiveMemoWithPeerList{ memo, peerlist, from_slabref } ).wait()
   }
}
