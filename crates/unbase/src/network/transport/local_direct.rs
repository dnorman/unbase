use super::*;
use crate::slab::*;
use std::{
    sync::{
        mpsc,
        Arc,
        Mutex,
    },
    thread,
};
use tracing::{
    debug,
    span,
    Level,
};

#[derive(Clone)]
pub struct LocalDirect {
    shared: Arc<Mutex<Internal>>,
}
struct Internal {
    tx_threads: Vec<thread::JoinHandle<()>>,
}

impl LocalDirect {
    // TODO: Potentially, make this return an Arc of itself.
    pub fn new() -> Self {
        LocalDirect { shared: Arc::new(Mutex::new(Internal { tx_threads: Vec::new() })), }
    }
}

impl Transport for LocalDirect {
    fn is_local(&self) -> bool {
        true
    }

    #[tracing::instrument]
    fn make_transmitter(&self, args: &TransmitterArgs) -> Option<Transmitter> {
        if let &TransmitterArgs::Local(rcv_slab) = args {
            let slab = rcv_slab.clone();
            let (tx_channel, rx_channel) = mpsc::channel::<(SlabRef, MemoRef)>();

            let span = span!(Level::TRACE, "LocalDirect Transmitter");
            let tx_thread: thread::JoinHandle<()> = thread::spawn(move || {
                let _guard = span.enter();

                // let mut buf = [0; 65536];
                debug!("Starting consumer");
                while let Ok((from_slabref, memoref)) = rx_channel.recv() {
                    debug!("LocalDirect Slab({}) RECEIVED {:?} from {}",
                           slab.my_ref, memoref, from_slabref.slab_id);
                    // clone_for_slab adds the memo to the slab, because memos cannot exist outside of an owning slab

                    let owned_slabref = slab.agent.localize_slabref(&from_slabref);
                    slab.agent.localize_memoref(&memoref, &owned_slabref, true);
                }
                debug!("Finished consumer");
            });

            // TODO: Remove the mutex here. Consider moving transmitter out of slabref.
            //       Instead, have relevant parties request a transmitter clone from the network
            self.shared.lock().unwrap().tx_threads.push(tx_thread);
            Some(Transmitter::new_local(args.get_slab_id(), Mutex::new(tx_channel)))
        } else {
            None
        }
    }

    fn bind_network(&self, _net: &Network) {}

    fn unbind_network(&self, _net: &Network) {}

    fn get_return_address(&self, address: &TransportAddress) -> Result<TransportAddress, Error> {
        if let TransportAddress::Local = *address {
            Ok(TransportAddress::Local)
        } else {
            Err(Error::BadAddress)
        }
    }
}

impl Drop for Internal {
    fn drop(&mut self) {
        for thread in self.tx_threads.drain(..) {
            debug!("# LocalDirectInternal.drop Thread pre join");
            thread.join().expect("local_direct thread join");
            debug!("# LocalDirectInternal.drop Thread post join");
        }
    }
}

impl std::fmt::Debug for LocalDirect {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("LocalDirect").finish()
    }
}
