
use ::futures::Executor;
use std::thread;


/// A Dispatcher is associated with a slab, and is notified via channel immediately after memos are written to slab storage.
/// Dispatcher is responsible for peering and other response behaviors, as well as notifying any parties waiting for said memo
pub struct Dispatcher{
    pub tx: mpsc::UnboundedSender<Dispatch>,
    worker_thread: thread::JoinHandle<()>
}


impl DispatcherThread{
    pub fn new ( net: Network, storage: StorageRequester, _counter: Arc<SlabCounter> ) -> DispatcherThread {

        let (tx,rx) = mpsc::unbounded::<Dispatch>();

        let mut inner = DispatcherInner{
            storage,
            memo_wait_channels:    HashMap::new(),
            subject_subscriptions: HashMap::new(),
            index_subscriptions:   Vec::new(),
            net,
        };

        let worker_thread = thread::spawn(move || {
            Executor::spawn(|_| {

                let server = rx.for_each(move |dispatch| {
                    current_thread::spawn( inner.dispatch( dispatch ) );
                    future::result(Ok(()))
                });

                Executor::spawn(server);

            });
        });

        Dispatcher{ tx, worker_thread }
    }
}