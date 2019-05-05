
use futures::{future, Stream, Future, channel::{mpsc,oneshot}};
//use ::executor::Executor;
use std::{thread,sync::{Arc,Mutex}};

use tokio::prelude::*;
use tokio::timer::Deadline;
use std::time::{Duration, Instant};

use crate::error::*;

enum WorkerMessage<Request,Response>{
    Shutdown,
    Request(Request, oneshot::Sender<Response>)
}

pub (crate) trait Worker {
    type Request;
    type Response;
    fn handle_message(&self, message: WorkerMessage<Self::Request, Self::Response>);
}

pub struct WorkerAgent<T> where T: Worker  {
    tx: mpsc::Sender<WorkerMessage<T::Request, T::Response>>,
    joinhandle: thread::JoinHandle<()>
}

impl <T> WorkerAgent <T> where T: Worker {
    pub fn new<T> (worker: T) -> WorkerAgent<T> {

        let (tx, rx) = mpsc::channel::<WorkerMessage<T::Request,T::Response>>(1024);

        let joinhandle = thread::spawn(move || {
            // TODO: Update this to use modular executors
            tokio::run(move |_| {
                let worker_task = rx.for_each(move |(me, resp_channel)| {
                    match message {
                        WorkerMessage::Request((request,resp_channel)) => {
                            Executor::spawn( worker.dispatch_request(r,resp_channel) );
                            future::result(Ok(()))
                        },
                        WorkerMessage::Shutdown => {
                            println!("Worker received shutdown");
                            future::result(Err(()))
                        }
                    }
                });

                tokio::spawn(worker_task);

            });
        });

        WorkerAgent { joinhandle, tx }

    }
    pub fn shutdown(&self) {
        println!("Sending shutdown to worker");
        self.tx.send(WorkerMessage::Shutdown);
        println!("Waiting for worker thread to exit");
        self.joinhandle.join();
        println!("worker thread exited - worker shutdown complete");
    }
    pub fn requester (&self) -> WorkerRequester<T::Request,T::Response> {
        WorkerRequester{ tx: self.tx.clone() }
    }
}

#[derive(Clone)]
pub (crate) struct WorkerRequester<Request,Response> {
    tx: mpsc::Sender<WorkerMessage<Request, Response>>,
}

impl WorkerRequester<Request, Response> {
    pub fn request (&self, request: Request ) -> Box<Future<Item=Response, Error=Error>> {
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
    pub fn request_timeout(&self, request: Request, timeout_ms: u64 ) -> Box<Future<Item=Response, Error=Error>> {

        let when = Instant::now() + Duration::from_millis(timeout_ms );

        Deadline::new( self.request( request ),when)
            .map_err(|e| Error::RetrieveError(RetrieveError::NotFoundByDeadline))
    }
}