use std::net::UdpSocket;
use std::thread;
use std::str;
use futures::future;
use futures::prelude::*;

use super::*;
use std::sync::mpsc;
use std::sync::{Arc,Mutex};
use slab::*;
use error::*;

// use std::collections::BTreeMap;
use super::packet::*;
use util::serde::DeserializeSeed;

use util::serde::{SerializeHelper,SerializeWrapper};
use super::packet::serde::PacketSeed;
//use std::time;

use serde_json;// {serialize as bin_serialize, deserialize as bin_deserialize};

#[derive(Clone)]
pub struct TransportUDP {
    shared: Arc<Mutex<TransportUDPInternal>>,
    // TEMPORARY - TODO: remove Arc<Mutex<>> here and instead make transmitters Send but not sync
}
struct TransportUDPInternal {
    socket: Arc<UdpSocket>,
    tx_thread: Option<thread::JoinHandle<()>>,
    rx_thread: Option<thread::JoinHandle<()>>,
    tx_channel: Option<Arc<Mutex<Option<mpsc::Sender<(TransportAddressUDP,Packet)>>>>>,
    network: Option<WeakNetwork>,
    address: TransportAddressUDP
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
                    socket: socket,
                    rx_thread: None,
                    tx_thread: Some(tx_thread),
                    tx_channel: Some(Arc::new(Mutex::new(Some(tx_channel)))),
                    network: None,
                    address: bind_address
                }
            ))
        }
    }

    fn setup_tx_thread (socket: Arc<UdpSocket>, inbound_address: TransportAddressUDP ) -> (thread::JoinHandle<()>,mpsc::Sender<(TransportAddressUDP, Packet)>){

        let (tx_channel, rx_channel) = mpsc::channel::<(TransportAddressUDP,Packet)>();

        let tx_thread : thread::JoinHandle<()> = thread::spawn(move || {

            let return_address = TransportAddress::UDP(inbound_address);
            //let mut buf = [0; 65536];
            while let Ok((to_address, packet)) = rx_channel.recv() {

                let helper = SerializeHelper {
                    return_address: &return_address,
                    dest_slab_id:   &packet.to_slab_id,
                };

                // KEEP THIS - This is the most useful memo trace
                //println!("UDP SEND FROM {} TO {} -> {}: {:?} {:?} {:?}", &packet.from_slab_id, &packet.to_slab_id, &packet.memo.id, &packet.memo.body, &packet.memo.parents.memo_ids(), &packet.peerlist.slab_ids() );
                let b = serde_json::to_vec( &SerializeWrapper(&packet, &helper) ).expect("serde_json::to_vec");
                //println!("UDP SEND {}", String::from_utf8(b.clone()).unwrap());

                //HACK: we're trusting that each memo is smaller than 64k
                socket.send_to(&b, &to_address.address).expect("Failed to send");
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
                address: TransportAddress::UDP( my_address.clone() ),
                lifetime: SlabAnticipatedLifetime::Unknown
            };

            let hello = slab.new_memo_basic_noparent(
                None,
                MemoBody::SlabPresence{ p: presence, r: net.get_root_index_seed(&slab) }
            );

            self.send_to_addr(
                slab,
                hello,
                to_address.clone()
            );
        }

    }
    pub fn send_to_addr (&self, from_slab: LocalSlabHandle, memoref: MemoRef, address : TransportAddressUDP) {

        // HACK - should actually retrieve the memo and sent it
        //        will require nonblocking retrieval mode
        if let Some(memo) = memoref.get_memo_if_resident() {
            let packet = Packet{
                to_slab_id: SlabRef::unknown(&from_slab).slab_id(),
                from_slabref: from_slab.slabref.clone(),
                memo: memo.clone(),
                peerstate: from_slab.get_peerstate(memoref, None).unwrap(),
            };

            //println!("TransportUDP.send({:?})", packet );

            if let Some(ref tx_channel) = self.shared.lock().unwrap().tx_channel {
                if let Some(ref tx_channel) = *(tx_channel.lock().unwrap()) {
                    tx_channel.send( (address, packet) ).unwrap();
                }
            }
        }
    }
}

impl Transport for TransportUDP {
    fn is_local (&self) -> bool {
        false
    }
    fn make_transmitter (&self, args: TransmitterArgs) -> Option<Transmitter> {

        if let TransmitterArgs::Remote(slabref,address) = args {
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

    fn bind_network(&self, net: &Network) {

        let mut shared = self.shared.lock().unwrap();
        if let Some(_) = (*shared).rx_thread {
            panic!("already bound to network");
        }

        let rx_socket = shared.socket.clone();
        //let dispatcher = TransportUDPDispatcher::new(net.clone());

        let net_weak = net.weak();
        let rx_handle : thread::JoinHandle<()> = thread::spawn(move || {
            let mut buf = [0; 65536];

            while let Ok((amt, src)) = rx_socket.recv_from(&mut buf) {

                if let Some(net) = net_weak.upgrade() {

                    //TODO: create a protocol encode/decode module and abstract away the serde stuff
                    //ouch, my brain - I Think I finally understand ser::de::DeserializeSeed
                    // println!("DESERIALIZE          {}", String::from_utf8(buf.to_vec()).unwrap());
                    let mut deserializer = serde_json::Deserializer::from_slice(&buf[0..amt]);

                    let packet_seed : PacketSeed = PacketSeed{
                        net: &net,
                        source_address: TransportAddress::UDP(TransportAddressUDP{ address: src.to_string() })
                    };

                    match packet_seed.deserialize(&mut deserializer) {
                        Ok(()) => {
                            // PacketSeed actually does everything
                        },
                        Err(e) =>{
                            println!("DESERIALIZE ERROR {}", e);
                        }
                    }
                    //println!("DESERIALIZE COMPLETE {}", String::from_utf8(buf.to_vec()).unwrap());
                }
            };
        });

        shared.rx_thread = Some(rx_handle);
        shared.network = Some(net.weak());

    }

    fn unbind_network(&self, _net: &Network) {
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
    tx_channel: Arc<Mutex<Option<mpsc::Sender<(TransportAddressUDP,Packet)>>>>
}
impl DynamicDispatchTransmitter for TransmitterUDP {
    fn send (&self, memo: Memo, peerstate: Vec<MemoPeerState>, from_slabref: SlabRef) -> Box<Future<Item=(), Error=Error>> {
        //println!("TransmitterUDP.send({:?},{:?})", from, memoref);

        let packet = Packet {
            to_slab_id: self.slab_id,
            from_slabref: from_slabref,
            memo:      memo,
            peerstate:  peerstate,
        };

        //println!("UDP QUEUE FOR SEND {:?}", &packet);

        //use util::serde::SerializeHelper;
        //let helper = SerializeHelper{ transmitter: self };
        //wrapper = SerializeWrapper<Packet>
//        let b = serde_json::to_vec(&packet).expect("serde_json::to_vec");
//        println!("UDP QUEUE FOR SEND SERIALIZED {}", String::from_utf8(b).unwrap() );

        //TODO: Convert this to use a tokio-shared-udp-socket or tokio_kcp
        let tx_channel = *(self.tx_channel.lock().unwrap()).as_ref().unwrap();
        tx_channel.send((self.address.clone(), packet)).unwrap();
        Box::new(future::result(Ok(())))
    }
}
impl Drop for TransmitterUDP{
    fn drop(&mut self) {
        //println!("# TransmitterUDP.drop");
    }
}
