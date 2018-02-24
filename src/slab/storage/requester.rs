use futures::{future, Future, sync::oneshot};
use super::*;
use error::*;

impl StorageRequester {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<LocalSlabRequestAndResponder>) {
        let (tx, rx) = mpsc::unbounded::<LocalSlabRequestAndResponder>();
        (StorageRequester { tx }, rx)
    }
    fn call (&self, request: LocalSlabRequest ) -> Box<Future<Item=LocalSlabResponse, Error=Error>> {
        let (p, c) = oneshot::channel::<Result<LocalSlabResponse,Error>>();
        self.tx.unbounded_send((request,p)).unwrap();

        Box::new( c.then(|r|{
            match r{
                Err(_) => {
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
}

impl StorageCoreInterface for StorageRequester {
    fn get_memo(&mut self, memoref: MemoRef, allow_remote: bool ) -> Box<Future<Item=Memo, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetMemo{ memoref, allow_remote } ).and_then(|r| {
            if let LocalSlabResponse::GetMemo(memo) = r {
                return Ok(memo)
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    fn put_memo(&mut self, memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef ) -> Box<Future<Item=MemoRef, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::PutMemo{ memo, peerset, from_slabref } ).and_then(|r| {
            if let LocalSlabResponse::PutMemo(memoref) = r {
                return Ok(memoref);
            }else{
                panic!("Invalid return type");
            }
        }))
    }
        fn put_memoref( &mut self, memo_id: MemoId, subject_id: SubjectId, peerset: MemoPeerSet) -> Box<Future<Item=MemoRef, Error=Error>> {
            // TODO: Implement
            unimplemented!()
        }
    fn send_memos ( &mut self, to_slabrefs: &[SlabRef], memorefs: &[MemoRef] ) -> Box<Future<Item=(), Error=Error>>{
        Box::new( self.call(LocalSlabRequest::SendMemo{ to_slabrefs: to_slabrefs.to_vec(), memorefs: memorefs.to_vec() } ).and_then(|r| {
            if let LocalSlabResponse::SendMemo(_) = r {
                return Ok(())
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    fn put_slab_presence (&mut self, presence: SlabPresence ) -> Box<Future<Item=(), Error=Error>>{
        Box::new( self.call(LocalSlabRequest::PutSlabPresence{ presence } ).and_then(|r| {
            if let LocalSlabResponse::SendMemo(_) = r {
                return Ok(())
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    fn get_slab_presence (&mut self, slabrefs: Vec<SlabRef>) -> Box<Future<Item=Vec<SlabPresence>, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetSlabPresence{ slabrefs } ).and_then(|r| {
            if let LocalSlabResponse::GetSlabPresence(presences) = r {
                return Ok(presences)
            }else{
                panic!("Invalid return type");
            }
        }))
    }
    fn get_peerset (&mut self, memorefs: Vec<MemoRef>, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerSet>, Error=Error>>{
        Box::new( self.call(LocalSlabRequest::GetPeerSet{ memorefs, maybe_dest_slabref } ).and_then(|r| {
            if let LocalSlabResponse::GetPeerSet(peersets) = r {
                return Ok(peersets)
            }else{
                panic!("Invalid return type");
            }
        }))
    }
}