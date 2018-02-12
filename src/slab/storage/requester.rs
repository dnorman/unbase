use futures::{future, Stream, Future, sync::{mpsc,oneshot}};
use super::*;
use error::*;

impl StorageRequester{
    fn call (&self, request: LocalSlabRequest ) -> Box<Future<Item=LocalSlabResponse, Error=Error>> {
        let (p, c) = oneshot::channel::<Result<LocalSlabResponse,Error>>();
        self.tx.unbounded_send((request,p)).unwrap();

        Box::new( c.then(|r|{
            match r{
                Err(e) => {
                    // The oneshot channel can fail if the sender goes away
                    future::result(Err(Error::LocalSlab(LocalSlabError::Unreachable)))
                }
                Ok(r) =>{
                    // The request can succeed or fail
                    future::result(r)
                }
            }
        }))
    }
    pub fn get_memo(&self, memoref: MemoRef ) -> Box<Future<Item=Option<Memo>, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetMemo{ memoref } ).and_then(|r| {
            if let LocalSlabResponse::GetMemo(maybe_memo) = r {
                return Ok(maybe_memo)
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    pub fn put_memo(&self, memo: Memo, peerstate: Vec<MemoPeerState>, from_slabref: SlabRef ) -> Box<Future<Item=(), Error=Error>>{
        Box::new( self.call(LocalSlabRequest::PutMemo{ memo, peerstate, from_slabref } ).and_then(|r| {
            if let LocalSlabResponse::PutMemo(_) = r {
                return Ok(())
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    pub fn send_memo ( &self, to_slabref: SlabRef, memoref: MemoRef ) -> Box<Future<Item=(), Error=Error>>{
        Box::new( self.call(LocalSlabRequest::SendMemo{ to_slabref, memoref } ).and_then(|r| {
            if let LocalSlabResponse::SendMemo(_) = r {
                return Ok(())
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    pub fn put_slab_presence (&self, presence: SlabPresence ) -> Box<Future<Item=(), Error=Error>>{
        Box::new( self.call(LocalSlabRequest::PutSlabPresence{ presence } ).and_then(|r| {
            if let LocalSlabResponse::SendMemo(_) = r {
                return Ok(())
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    pub fn get_peerstate (&self, memoref: MemoRef, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerState>, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetPeerState{ memoref, maybe_dest_slabref } ).and_then(|r| {
            if let LocalSlabResponse::GetPeerState(peerstate) = r {
                return Ok(peerstate)
            }else{
                panic!("Invalid return type");
            }
        }))
    }
}