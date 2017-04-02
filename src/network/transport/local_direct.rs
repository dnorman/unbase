use std::sync::{Arc,Mutex};
use std::thread;
use std::sync::mpsc;
use slab::memo::*;
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
            let slab = rcv_slab.weak();
            let (tx_channel, rx_channel) = mpsc::channel::<(SlabRef,MemoPeeringStatus,Memo)>();

            let tx_thread : thread::JoinHandle<()> = thread::spawn(move || {
                //let mut buf = [0; 65536];
                //println!("Started TX Thread");g
                while let Ok((from_slabref, from_slab_peering_status, memo)) = rx_channel.recv() {
                    //println!("CHANNEL RCV {:?}", memo);
                    if let Some(slab) = slab.upgrade(){
                        slab.put_memo(&MemoOrigin::OtherSlab(&from_slabref,from_slab_peering_status), memo);
                    }
                }
            });

            // TODO: Remove the mutex here. Consider moving transmitter out of slabref.
            //       Instead, have relevant parties request a transmitter clone from the network
            self.shared.lock().unwrap().tx_threads.push(tx_thread);
            Some(Transmitter::new_local(Mutex::new(tx_channel)))
        }else{
            None
        }

    }

    fn bind_network(&self, _net: &Network) {}
    fn unbind_network(&self, _net: &Network) {}

    fn get_return_address  ( &self, address: &TransportAddress ) -> Option<TransportAddress> {
        if let TransportAddress::Local = *address {
            Some(TransportAddress::Local)
        }else{
            None
        }
    }
}

impl Drop for Internal {
    fn drop (&mut self) {
        println!("# LocalDirectInternal.drop");
        for _thread in self.tx_threads.drain(..) {

            println!("# LocalDirectInternal.drop Thread pre join");
            //TODO: Figure out why the transmitter isn't getting freed
            //      because we're getting stuck here
            //thread.join().unwrap();

            println!("# LocalDirectInternal.drop Thread post join");
        }
    }
}