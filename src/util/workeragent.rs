
use futures::{future, {Stream,Future}, sync::{mpsc,oneshot}};
use ::executor::Executor;
use std::thread;

enum WorkerMessage<Worker>{
    Shutdown,
    Request(Worker::Request, oneshot::Sender<Worker::Response>)
}

trait Worker {

}

pub struct WorkerAgent<T> where T: Worker  {
    sender: mpsc::UnboundedSender<WorkerMessage<T>>,
    joinhandle: thread::JoinHandle<()>
}

//
//struct CoreThreadInner<'a, T> where &'a mut T: SlabStorageHandle +Send+'a {
//    core: T
//}

impl <T> WorkerAgent <T> where T: Worker {
    pub fn new<T> (worker: T, request_rx:  mpsc::UnboundedReceiver<WorkerMessage<T>>) -> WorkerAgent<T> {
        let mut inner = CoreThreadInner{ core };

        let joinhandle = thread::spawn(move || {

            current_thread::run(|_| {
                // LEFT OFF HERE
                let server = request_rx.for_each(move |(request, resp_channel)| {
                    Executor::spawn( inner.dispatch_request(request,resp_channel) );
                    future::result(Ok(()))
                });

                Executor::spawn(server);

            });
        });

        StoreController { joinhandle }

    }
}
