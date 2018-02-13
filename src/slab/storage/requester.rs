use futures::{future, Stream, Future, sync::oneshot};
use super::*;
use error::*;

impl StorageRequester{
    pub fn new () -> (Self,mpsc::UnboundedReceiver<LocalSlabRequestAndResponder>) {
        let (tx,rx) = mpsc::unbounded::<LocalSlabRequestAndResponder>();
        ( StorageRequester{tx}, rx )
    }
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
    pub fn get_memo(&self, memoref: MemoRef ) -> Box<Future<Item=StorageMemoRetrieval, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetMemo{ memoref } ).and_then(|r| {
            if let LocalSlabResponse::GetMemo(retrieval) = r {
                return Ok(retrieval)
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    pub fn put_memo(&self, memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef ) -> Box<Future<Item=MemoRef, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::PutMemo{ memo, peerset, from_slabref } ).and_then(|r| {
            if let LocalSlabResponse::PutMemo(memoref) = r {
                return Ok(memoref);
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
    pub fn get_peerset (&self, memoref: MemoRef, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerState>, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetPeerSet{ memoref, maybe_dest_slabref } ).and_then(|r| {
            if let LocalSlabResponse::GetPeerSet(peerset) = r {
                return Ok(peerset)
            }else{
                panic!("Invalid return type");
            }
        }))
    }
}