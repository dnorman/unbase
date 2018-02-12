use futures::{future, {Stream,Future}, sync::{mpsc,oneshot}};
use std::thread;
use tokio::executor::current_thread;

use super::*;

pub struct CoreThread{
    tx: mpsc::UnboundedSender<(LocalSlabRequest,oneshot::Sender<Result<LocalSlabResponse,Error>>)>,
    worker_thread: thread::JoinHandle<()>
}

struct CoreThreadInner{
    core: Box<StorageInterfaceCore+Send> 
}

impl CoreThread{
    pub fn new (core: Box<StorageInterfaceCore+Send> ) -> CoreThread {

        let (tx,rx) = mpsc::unbounded::<(LocalSlabRequest,oneshot::Sender<Result<LocalSlabResponse,Error>>)>();

        let inner = CoreThreadInner{ core };

        let worker_thread = thread::spawn(move || {
            current_thread::run(|_| {

            let server = rx.for_each(|(request, resp_channel)| {
                current_thread::spawn( inner.dispatch_request(request,resp_channel) );
                future::result(Ok(()))
            });

            current_thread::spawn(server);

            });
        });

        CoreThread{ tx, worker_thread }

    }
    pub fn requester(&self) -> StorageRequester {
        StorageRequester{ tx: self.tx.clone() }
    }
}

impl CoreThreadInner{
    fn dispatch_request(&self,request: LocalSlabRequest, responder: oneshot::Sender<Result<LocalSlabResponse,Error>>) -> Box<Future<Item=(), Error=()>> {
        use slab::storage::LocalSlabRequest::*;

        // NFI how to do match statements which return different futures
        if let GetMemo{ memoref } = request {
            return Box::new( self.core.get_memo(memoref).then(|r| {
                match r {
                    Ok(maybe_memo)  => responder.send(Ok(LocalSlabResponse::GetMemo( maybe_memo ) )),
                    Err(e)          => responder.send(Err(e))
                };
                Ok(())
            }))
        }
        if let PutMemo { memo, peerstate, from_slabref } = request{
            return Box::new(self.core.put_memo( memo, peerstate, from_slabref ).then(|r| {
                match r {
                    Ok(r)  => responder.send(Ok(LocalSlabResponse::PutMemo( () ) )),
                    Err(e) => responder.send(Err(e))
                };
                Ok(())
            }))
        }

        if let SendMemo {to_slabref, memoref} = request{
            return Box::new(self.core.send_memo(to_slabref, memoref).then(|r| {
                match r {
                    Ok(r)  => responder.send(Ok(LocalSlabResponse::SendMemo( () ) )),
                    Err(e) => responder.send(Err(e))
                };
                Ok(())
            }))
        }

        panic!("didn't implement handler for memo request type" )
    }
}