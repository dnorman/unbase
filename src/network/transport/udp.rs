use std::net::UdpSocket;
use std::thread;
use std::str;
use futures::future;
use futures::prelude::*;
use buffer::{NetworkBuffer,receiver::NetworkReceiver};

use super::*;
use std::sync::mpsc;
use std::sync::{Arc,Mutex};
use slab::*;
use error::*;

// use std::collections::BTreeMap;
//use std::time;

use subject::SubjectId;// {serialize as bin_serialize, deserialize as bin_deserialize};

#[derive(Clone)]
pub struct TransportUDP {
    shared: Arc<Mutex<TransportUDPInternal>>,
    // TEMPORARY - TODO: remove Arc<Mutex<>> here and instead make transmitters Send but not sync
}
struct TransportUDPInternal {
    socket: Arc<UdpSocket>,
    tx_thread: Option<thread::JoinHandle<()>>,
    rx_thread: Option<thread::JoinHandle<()>>,
    tx_channel: Option<Arc<Mutex<Option<mpsc::Sender<(TransportAddressUDP,NetworkBuffer)>>>>>,
    network: Option<WeakNetwork>,
    address: TransportAddressUDP
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub struct TransportAddressUDP {
    address: String
}
impl TransportAddressUDP {
    pub fn to_string (&self) -> String {
        "udp:".to_string() + &self.address
    }
}

impl TransportUDP {

    /// UDP Transport
    /// 
    /// ```
    /// use std::{thread,time};
    /// use unbase::{Network,Slab,SubjectHandle};
    /// use unbase::network::transport::TransportUDP;
    /// thread::spawn(|| {
    ///     let net1 = Network::create_new_system();
    ///     let udp1 = TransportUDP::new("127.0.0.1:12021".to_string());
    ///     net1.add_transport( Box::new(udp1) );
    ///     let context_a = Slab::new(&net1).create_context();
    ///
    ///     // HACK - wait for slab_b to be on the peer list, and to be hooked in to our root_index_seed
    ///     thread::sleep( time::Duration::from_millis(150) );
    ///     let beast_a = SubjectHandle::new_kv(&context_a, "beast", "Lion").expect("write successful");
    ///     beast_a.set_value("sound","Grraaawrrr").expect("write successful");
    ///
    ///     // Hang out so we can help thread 2
    ///     thread::sleep(time::Duration::from_millis(500));
    /// });
    
    /// // HACK - Ensure slab_a is listening
    /// thread::sleep( time::Duration::from_millis(20) );
    
    /// thread::spawn(|| {
    ///      let net2 = Network::new();
    ///      net2.hack_set_next_slab_id(200);
    ///      let udp2 = TransportUDP::new("127.0.0.1:12022".to_string());
    ///     net2.add_transport( Box::new(udp2.clone()) );
    ///     let slab_b = Slab::new(&net2);
    ///     udp2.seed_address_from_string( "127.0.0.1:12021".to_string() );
    ///     let context_b = slab_b.create_context();
    ///     let beast_b = context_b.fetch_kv_wait("beast","Lion",1000).expect("it worked");
    ///     println!("The {} goes {}", beast_b.get_value("beast").expect("it worked"), beast_b.get_value("sound").expect("it worked") )
    /// });
    /// ```
    pub fn new (address: String) -> Self{

        let bind_address = TransportAddressUDP{ address : address };

        let socket = Arc::new( UdpSocket::bind( bind_address.address.clone() ).expect("UdpSocket::bind") );
        //socket.set_read_timeout( Some(time::Duration::from_millis(2000)) ).expect("set_read_timeout call failed");

        let (tx_thread,tx_channel) = Self::setup_tx_thread(socket.clone(), bind_address.clone());

        TransportUDP {
            shared: Arc::new(Mutex::new(
                TransportUDPInternal {
                    socket,
                    rx_thread: None,
                    tx_thread: Some(tx_thread),
                    tx_channel: Some(Arc::new(Mutex::new(Some(tx_channel)))),
                    network: None,
                    address: bind_address
                }
            ))
        }
    }

    fn setup_tx_thread (socket: Arc<UdpSocket>, inbound_address: TransportAddressUDP ) -> (thread::JoinHandle<()>,mpsc::Sender<(TransportAddressUDP, NetworkBuffer)>){

        let (tx_channel, rx_channel) = mpsc::channel::<(TransportAddressUDP,NetworkBuffer)>();

        let tx_thread : thread::JoinHandle<()> = thread::spawn(move || {

            let _return_address = TransportAddress::UDP(inbound_address);
            //let mut buf = [0; 65536];
            while let Ok((to_address, buf)) = rx_channel.recv() {
                //HACK: we're trusting that each memo is smaller than 64k
                socket.send_to(&buf.to_vec(), &to_address.address).expect("Failed to send");
                //println!("SENT UDP PACKET ({}) {}", &to_address.address, &String::from_utf8(b).unwrap());
            }
    });

        (tx_thread, tx_channel)
    }
    pub fn seed_address_from_string (&self, address_string: String) {

        let to_address = TransportAddressUDP{ address: address_string };

        let net;
        let my_address;
        {
            let shared = self.shared.lock().expect("TransportUDP.shared.lock");
            my_address = shared.address.clone();

            if let Some(ref n) = shared.network {
                net = n.upgrade().expect("Network upgrade");
            }else{
                panic!("Attempt to use uninitialized transport");
            }
        };

        for slab in net.get_all_local_slab_handles() {

            let presence = SlabPresence {
                slab_id: slab.slabref.slab_id(),
                addresses: vec![TransportAddress::UDP( my_address.clone() )],
                lifetime: SlabAnticipatedLifetime::Unknown
            };

            let hello = slab.new_memo_basic_noparent(
                SubjectId::anonymous(),
                MemoBody::SlabPresence{ p: presence, r: net.get_root_index_seed(&slab) }
            );

            self.send_to_addr(
                slab,
                hello,
                to_address.clone()
            );
        }

    }
    pub fn send_to_addr(&self, _from_slab: LocalSlabHandle, _memoref: MemoRef, _address : TransportAddressUDP) -> Box<Future<Item=(),Error=Error>> {

        // HACK - should actually retrieve the memo and sent it
        //        will require nonblocking retrieval mode

//        Box::new(from_slab.get_memo(memoref,false).and_then(|memo| {
//            let buf = Packet {
//                to_slab_id: SlabRef::unknown(&from_slab).slab_id(),
//                from_slabref: from_slab.slabref.clone(),
//                memo: memo.buffer(),
//                peerset: from_slab.get_peerset(memoref, None).unwrap(),
//            }.buffer();
//
//            //println!("TransportUDP.send({:?})", packet );
//
//            unimplemented!()
////            if let Some(ref tx_channel) = self.shared.lock().unwrap().tx_channel {
////                if let Some(ref tx_channel) = *(tx_channel.lock().unwrap()) {
////                    tx_channel.send((address, buf)).unwrap();
////                }
////            }
////
////            Ok(())
//        }))
        unimplemented!()
    }
}

impl Transport for TransportUDP {
    fn make_transmitter (&self, args: &TransmitterArgs) -> Option<Transmitter> {

        if let &TransmitterArgs::Remote(slabref,address) = args {
            if let &TransportAddress::UDP(ref udp_address) = address {

                if let Some(ref tx_channel) = self.shared.lock().unwrap().tx_channel {

                    let tx = TransmitterUDP{
                        slab_id: slabref.slab_id(),
                        address: udp_address.clone(),
                        tx_channel: tx_channel.clone(),
                    };

                    return Some(Transmitter::new( args.get_slabref(), Box::new(tx) ))
                }
            }
        }
        None
    }
    fn is_local (&self) -> bool {
        false
    }
    fn bind_network(&mut self, net: &Network) {

        let mut shared = self.shared.lock().unwrap();
        if let Some(_) = (*shared).rx_thread {
            panic!("already bound to network");
        }

        let rx_socket = shared.socket.clone();
        let receiver = NetworkReceiver::new(net);

        let rx_handle : thread::JoinHandle<()> = thread::spawn(move || {
            let mut buf = [0; 0x1_0000];

            while let Ok((amt, src)) = rx_socket.recv_from(&mut buf) {
                let source_address = TransportAddress::UDP(TransportAddressUDP{ address: src.to_string() });

                let buffer = NetworkBuffer::from_slice(&buf[0..amt]);
                buffer.extract_to(receiver);
            };
        });

        shared.rx_thread = Some(rx_handle);
        shared.network = Some(net.weak());

    }

    fn unbind_network(&mut self, _net: &Network) {
        /*,
        let mut shared = self.shared.lock().unwrap();
        shared.tx_thread = None;
        shared.rx_thread = None;
        shared.tx_channel = None;
        shared.network = None;
        */
    }
    fn get_return_address  ( &self, address: &TransportAddress ) -> TransportAddress {
        if let TransportAddress::UDP(_) = *address {
            let shared = self.shared.lock().unwrap();
            TransportAddress::UDP(shared.address.clone())
        }else{
            TransportAddress::Blackhole
        }
    }
}

impl Drop for TransportUDPInternal{
    fn drop(&mut self) {
        //println!("# TransportUDPInternal().drop");

        // BUG NOTE: having to use a pretty extraordinary workaround here
        //           this horoughly horrible Option<Arc<Mutex<Option<>>> regime
        //           is necessary because the tx_thread was somehow being wedged open
        //           presumably we have transmitters that aren't going out of scope somewhere
        if let Some(ref tx) = self.tx_channel {
            tx.lock().unwrap().take();
        }

        self.tx_thread.take().unwrap().join().unwrap();

        //TODO: implement an atomic boolean and a timeout to close the receiver thread in an orderly fashion
        //self.rx_thread.take().unwrap().join().unwrap();

        // TODO: Drop all observers? Or perhaps observers should drop the slab (weak ref directionality)
    }
}

pub struct TransmitterUDP{
    pub slab_id: slab::SlabId,
    address: TransportAddressUDP,
    // HACK HACK HACK - lose the Arc<Mutex<>> here by making transmitter Send, but not Sync
    tx_channel: Arc<Mutex<Option<mpsc::Sender<(TransportAddressUDP,NetworkBuffer)>>>>
}
impl DynamicDispatchTransmitter for TransmitterUDP {
    fn send (&self, buffer: NetworkBuffer) -> future::FutureResult<(), Error> {
        //println!("TransmitterUDP.send({:?},{:?})", from, memoref);

        //let buf = NetworkBuffer::single(memo,peerset,from_slabref);

//        let _buf = Packet {
//            to_slab_id: self.slab_id,
//            from_slab_id: from_slabref.slab_id(),
//            memo: memo,
//            peerset,
//        }.buffer();

//        println!("UDP QUEUE FOR SEND SERIALIZED {}", String::from_utf8(b).unwrap() );

        //TODO: Convert this to use a tokio-shared-udp-socket or tokio_kcp
        let tx_channel = *(self.tx_channel.lock().unwrap()).as_ref().unwrap();
        tx_channel.send((self.address.clone(), buffer.to_vec())).unwrap();

        future::result(Ok(()))
    }
}
impl Drop for TransmitterUDP{
    fn drop(&mut self) {
        //println!("# TransmitterUDP.drop");
    }
}
