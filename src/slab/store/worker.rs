
use util::workeragent::Worker;
use super::*;

struct SlabStoreWorker{

}

impl <T> Worker<T> for SlabStoreWorker where T: SlabStore {
    type Request = LocalSlabRequest;
    type Response = Result<LocalSlabResponse,Error>;

    fn handle_message(&mut self, message: WorkerMessage<Self>) -> Box<Future<Item=(), Error=()>> {
        use slab::storage::LocalSlabRequest::*;

        // NFI how to do match statements which return different futures
        if let GetMemo{ memoref, allow_remote } = request {
            return Box::new( (&mut self.core).get_memo(memoref, allow_remote).then(|r| {
                match r {
                    Ok(maybe_memo)  => responder.send(Ok(LocalSlabResponse::GetMemo( maybe_memo ) )),
                    Err(e)          => responder.send(Err(e))
                }.unwrap_or(());
                Ok(())
            }))
        }
        if let PutMemo { memo, peerset, from_slabref } = request{
            return Box::new(self.core.put_memo( memo, peerset, from_slabref ).then(|r| {
                match r {
                    Ok(memoref)  => responder.send(Ok(LocalSlabResponse::PutMemo( memoref ) )),
                    Err(e) => responder.send(Err(e))
                }.unwrap_or(());
                Ok(())
            }))
        }

        if let SendMemo {to_slabrefs, memorefs} = request{
            return Box::new(self.core.send_memos(&to_slabrefs, &memorefs).then(|r| {
                match r {
                    Ok(_)  => responder.send(Ok(LocalSlabResponse::SendMemo( () ) )),
                    Err(e) => responder.send(Err(e))
                }.unwrap_or(());
                Ok(())
            }))
        }

        if let GetPeerSet {memorefs, maybe_dest_slabref } = request{
            return Box::new(self.core.get_peerset(memorefs, maybe_dest_slabref).then(|r| {
                match r {
                    Ok(r)  => responder.send(Ok(LocalSlabResponse::GetPeerSet( r ) )),
                    Err(e) => responder.send(Err(e))
                }.unwrap_or(());
                Ok(())
            }))
        }

        panic!("didn't implement handler for memo request type" )
    }
}