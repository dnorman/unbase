use std::sync::{Arc,Mutex};
use std::thread;
use std::sync::mpsc;
use super::*;

#[derive(Clone)]
pub struct LocalDirect {
    shared: Arc<Mutex<Internal>>,
}
struct Internal {
    tx_threads: Vec<thread::JoinHandle<()>>
}

impl LocalDirect {
    // TODO: Potentially, make this return an Arc of itself.
    pub fn new () -> Self{
        LocalDirect {
            shared: Arc::new(Mutex::new(
                Internal {
                    tx_threads: Vec::new()
                }
            ))
        }
    }
}

impl Transport for LocalDirect {
    fn is_local (&self) -> bool {
        true
    }
    fn make_transmitter (&self, args: &TransmitterArgs ) -> Option<Transmitter> {
        if let &TransmitterArgs::Local(rcv_slab) = args {
            let (tx_channel, receiver) = mpsc::channel::<MemoRefFromSlab>();


            // TODO1 - LEFT off here
            // reconcile this code with memo::serde calling slab.reconstitute_memo
            // reconstitute_memo needs to be abstracted so it can function for network deserialization and/or disk

            // Deserialize from network to slab (via slabhandle)
            // Deserialize from disk/binary to... where?
            //rcv_slab.add_receiver(rx_channel).wait();

            let tx_thread : thread::JoinHandle<()> = thread::spawn(move || {
                //let mut buf = [0; 65536];
                //println!("Started TX Thread");
                while let Ok((from_slabref, memoref)) = rx_channel.recv() {
                    //println!("LocalDirect Slab({}) RECEIVED {:?} from {}", slab.id, memoref, from_slabref.slab_id);
                    // clone_for_slab adds the memo to the slab, because memos cannot exist outside of an owning slab

                    let owned_slabref = slab.get_slabref_for_slab_id( from_slabref.slab_id() )
                    memoref.clone_for_slab(&owned_slabref, &rcv_slab, true);
                }
            });

            // TODO: Remove the mutex here. Consider moving transmitter out of slabref.
            //       Instead, have relevant parties request a transmitter clone from the network
            self.shared.lock().unwrap().tx_threads.push(tx_thread);
            Some(Transmitter::new_local(args.get_slabref(), Mutex::new(tx_channel)))
        }else{
            None
        }

    }

    fn bind_network(&self, _net: &Network) {}
    fn unbind_network(&self, _net: &Network) {}

    fn get_return_address  ( &self, address: &TransportAddress ) -> TransportAddress {
        if let TransportAddress::Local = *address {
            TransportAddress::Local
        }else{
            TransportAddress::Blackhole
        }
    }
}

impl Drop for Internal {
    fn drop (&mut self) {
        // NOTE: it's kind of pointless to join our threads on drop
        //       Shut them down, sure, but not point to waiting for that to happen while we're dropping.
        //       Also this seems to have triggered a bug of some kind 
        
        //println!("# LocalDirectInternal.drop");
        //for thread in self.tx_threads.drain(..) {
            //println!("# LocalDirectInternal.drop Thread pre join");
            //thread.join().expect("local_direct thread join");
            //println!("# LocalDirectInternal.drop Thread post join");
        //}
    }
}
