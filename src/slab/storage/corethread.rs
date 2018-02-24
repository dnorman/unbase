use futures::{future, {Stream,Future}, sync::{mpsc,oneshot}};
use std::thread;
use tokio::executor::current_thread;

use super::*;

pub struct CoreThread{
    worker_thread: thread::JoinHandle<()>
}

struct CoreThreadInner{
    core: Box<StorageCoreInterface+Send> 
}

impl CoreThread{
    pub fn new (core: Box<StorageCoreInterface+Send>, request_rx:  mpsc::UnboundedReceiver<LocalSlabRequestAndResponder>) -> CoreThread {

        let mut inner = CoreThreadInner{ core };

        let worker_thread = thread::spawn(move || {
            current_thread::run(|_| {

            let server = request_rx.for_each(move |(request, resp_channel)| {
                current_thread::spawn( inner.dispatch_request(request,resp_channel) );
                future::result(Ok(()))
            });

            current_thread::spawn(server);

            });
        });

        CoreThread{ worker_thread }

    }
}

impl CoreThreadInner{
    fn dispatch_request(&mut self,request: LocalSlabRequest, responder: oneshot::Sender<Result<LocalSlabResponse,Error>>) -> Box<Future<Item=(), Error=()>> {
        use slab::storage::LocalSlabRequest::*;

        // NFI how to do match statements which return different futures
        if let GetMemo{ memoref, allow_remote } = request {
            return Box::new( self.core.get_memo(memoref, allow_remote).then(|r| {
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