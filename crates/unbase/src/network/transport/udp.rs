use crate::{
    head::Head,
    network::{
        transmitter::DynamicDispatchTransmitter,
        Network,
        SerdePacket,
        Transmitter,
        TransmitterArgs,
        Transport,
        TransportAddress,
        WeakNetwork,
    },
    slab::{
        MemoBody,
        MemoRef,
        SlabId,
        SlabPresence,
        SlabRef,
        TransportLiveness,
    },
    util::serde::{
        DeserializeSeed,
        SerializeHelper,
        SerializeWrapper,
    },
};

use std::{
    fmt,
    net::UdpSocket,
    sync::{
        mpsc,
        Arc,
        Mutex,
    },
    thread,
};

// use futures::{
//    TODO
//};

// use std::collections::BTreeMap;
use super::packet::serde::PacketSeed;
use tracing::{
    error,
    trace,
};

use crate::error::Error;
use serde_json;

#[derive(Clone)]
pub struct TransportUDP {
    shared: Arc<Mutex<TransportUDPInternal>>,
    // TEMPORARY - TODO: remove Arc<Mutex<>> here and instead make transmitters Send but not sync
}
struct TransportUDPInternal {
    socket:     Arc<UdpSocket>,
    tx_thread:  Option<thread::JoinHandle<()>>,
    rx_thread:  Option<thread::JoinHandle<()>>,
    tx_channel: Option<Arc<Mutex<Option<mpsc::Sender<(TransportAddressUDP, SerdePacket)>>>>>,
    network:    Option<WeakNetwork>,
    address:    TransportAddressUDP,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct TransportAddressUDP {
    address: String,
}
impl TransportAddressUDP {
    pub fn to_string(&self) -> String {
        "udp:".to_string() + &self.address
    }
}

impl TransportUDP {
    /// UDP Transport
    /// TODO: update this to use task spawn
    /// ```
    /// let net = unbase::Network::new();
    /// let udp = unbase::network::transport::TransportUDP::new("127.0.0.1:51002".to_string());
    /// net.add_transport(Box::new(udp.clone()));
    /// let slab = unbase::Slab::initialize(&net);
    /// let context = slab.create_context();
    ///
    /// udp.seed_address_from_string("127.0.0.1:51001".to_string());
    /// // do stuff with the context
    /// ```
    pub fn new(address: String) -> Self {
        let bind_address = TransportAddressUDP { address };

        let socket = Arc::new(UdpSocket::bind(bind_address.address.clone()).expect("UdpSocket::bind"));
        // socket.set_read_timeout( Some(time::Duration::from_millis(2000)) ).expect("set_read_timeout call failed");

        let (tx_thread, tx_channel) = Self::setup_tx_thread(socket.clone(), bind_address.clone());

        TransportUDP { shared: Arc::new(Mutex::new(TransportUDPInternal { socket,
                                                                          rx_thread: None,
                                                                          tx_thread: Some(tx_thread),
                                                                          tx_channel:
                                                                              Some(Arc::new(Mutex::new(Some(tx_channel)))),
                                                                          network: None,
                                                                          address: bind_address })), }
    }

    fn setup_tx_thread(socket: Arc<UdpSocket>, inbound_address: TransportAddressUDP)
                       -> (thread::JoinHandle<()>, mpsc::Sender<(TransportAddressUDP, SerdePacket)>) {
        let (tx_channel, rx_channel) = mpsc::channel::<(TransportAddressUDP, SerdePacket)>();

        let tx_thread: thread::JoinHandle<()> = thread::spawn(move || {
            let return_address = TransportAddress::UDP(inbound_address);
            // let mut buf = [0; 65536];
            while let Ok((to_address, packet)) = rx_channel.recv() {
                let helper = SerializeHelper { return_address: &return_address,
                                               dest_slab_id:   &packet.to_slab_id, };

                let b = serde_json::to_vec(&SerializeWrapper(&packet, &helper)).expect("serde_json::to_vec");

                trace!("UDP SEND FROM {} ({}) TO {} ({}): {}",
                       &packet.from_slab_id,
                       socket.local_addr().unwrap(),
                       packet.to_slab_id.map_or("ANY", |s| &s.short()),
                       &to_address.address,
                       String::from_utf8(b.clone()).unwrap());
                // HACK: we're trusting that each memo is smaller than 64k
                socket.send_to(&b, &to_address.address).expect("Failed to send");
            }
        });

        (tx_thread, tx_channel)
    }

    pub fn seed_address_from_string(&self, address_string: String) {
        let to_address = TransportAddressUDP { address: address_string };

        let net;
        let my_address;
        {
            let shared = self.shared.lock().expect("TransportUDP.shared.lock");
            my_address = shared.address.clone();

            if let Some(ref n) = shared.network {
                net = n.upgrade().expect("Network upgrade");
            } else {
                panic!("Attempt to use uninitialized transport");
            }
        };

        for slab in net.get_all_local_slabs() {
            let presence = SlabPresence { slab_id:  slab.my_ref.slab_id,
                                          address:  TransportAddress::UDP(my_address.clone()),
                                          liveness: TransportLiveness::Unknown, };

            let hello = slab.new_memo(None,
                                      Head::Null,
                                      MemoBody::SlabPresence { p: presence,
                                                               r: net.get_root_index_seed(&slab), });

            self.send_to_addr(&slab.my_ref, hello, to_address.clone());
        }
    }

    #[tracing::instrument]
    pub fn send_to_addr(&self, from_slabref: &SlabRef, memoref: MemoRef, address: TransportAddressUDP) {
        // HACK - should actually retrieve the memo and sent it
        //        will require nonblocking retrieval mode

        if let Some(memo) = from_slabref.get_memo_if_resident(memoref) {
            let packet = SerdePacket { to_slab_id:   None,
                                       from_slab_id: from_slabref.0.slab_id,
                                       memo:         memo.clone(),
                                       peerlist:     memoref.get_peerlist_for_peer(from_slabref, None), };

            if let Some(ref tx_channel) = self.shared.lock().unwrap().tx_channel {
                if let Some(ref tx_channel) = *(tx_channel.lock().unwrap()) {
                    tx_channel.send((address, packet)).unwrap();
                }
            }
        }
    }
}

impl fmt::Debug for TransportUDP {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TransportUDP")
           .field("address", &self.shared.lock().unwrap().address)
           .finish()
    }
}

impl Transport for TransportUDP {
    fn is_local(&self) -> bool {
        false
    }

    fn make_transmitter(&self, args: &TransmitterArgs) -> Option<Transmitter> {
        if let &TransmitterArgs::Remote(slab_id, address) = args {
            if let &TransportAddress::UDP(ref udp_address) = address {
                if let Some(ref tx_channel) = self.shared.lock().unwrap().tx_channel {
                    let tx = TransmitterUDP { slab_id:    *slab_id,
                                              address:    udp_address.clone(),
                                              tx_channel: tx_channel.clone(), };

                    return Some(Transmitter::new(args.get_slab_id(), Box::new(tx)));
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
        // let dispatcher = TransportUDPDispatcher::new(net.clone());

        let net_weak = net.weak();
        let rx_handle: thread::JoinHandle<()> = thread::spawn(move || {
            let mut buf = [0; 65536];

            let local_addr = rx_socket.local_addr().unwrap();

            while let Ok((amt, src)) = rx_socket.recv_from(&mut buf) {
                if let Some(net) = net_weak.upgrade() {
                    // TODO: create a protocol encode/decode module and abstract away the serde stuff
                    // ouch, my brain - I Think I finally understand ser::de::DeserializeSeed

                    tracing::info!("UDP RECV BY {} FROM {}: {}",
                                   local_addr,
                                   src,
                                   String::from_utf8(buf.to_vec()).unwrap());
                    let mut deserializer = serde_json::Deserializer::from_slice(&buf[0..amt]);

                    let packet_seed: PacketSeed =
                        PacketSeed { net:            &net,
                                     source_address: TransportAddress::UDP(TransportAddressUDP { address: src.to_string(), }), };

                    match packet_seed.deserialize(&mut deserializer) {
                        Ok(()) => {
                            // PacketSeed actually does everything
                        },
                        Err(e) => {
                            error!("DESERIALIZE ERROR {}", e);
                        },
                    }
                }
            }
        });

        shared.rx_thread = Some(rx_handle);
        shared.network = Some(net.weak());
    }

    fn unbind_network(&self, _net: &Network) {
        // ,
        // let mut shared = self.shared.lock().unwrap();
        // shared.tx_thread = None;
        // shared.rx_thread = None;
        // shared.tx_channel = None;
        // shared.network = None;
    }

    fn get_return_address(&self, address: &TransportAddress) -> Result<TransportAddress, Error> {
        if let TransportAddress::UDP(_) = *address {
            let shared = self.shared.lock().unwrap();
            Ok(TransportAddress::UDP(shared.address.clone()))
        } else {
            Err(Error::BadAddress)
        }
    }
}

impl Drop for TransportUDPInternal {
    fn drop(&mut self) {
        // BUG NOTE: having to use a pretty extraordinary workaround here
        //           this horoughly horrible Option<Arc<Mutex<Option<>>> regime
        //           is necessary because the tx_thread was somehow being wedged open
        //           presumably we have transmitters that aren't going out of scope somewhere
        if let Some(ref tx) = self.tx_channel {
            tx.lock().unwrap().take();
        }

        self.tx_thread.take().unwrap().join().unwrap();

        // TODO: implement an atomic boolean and a timeout to close the receiver thread in an orderly fashion
        // self.rx_thread.take().unwrap().join().unwrap();

        // TODO: Drop all observers? Or perhaps observers should drop the slab (weak ref directionality)
    }
}

pub struct TransmitterUDP {
    pub slab_id: SlabId,
    address:     TransportAddressUDP,
    // HACK HACK HACK - lose the Arc<Mutex<>> here by making transmitter Send, but not Sync
    tx_channel:  Arc<Mutex<Option<mpsc::Sender<(TransportAddressUDP, SerdePacket)>>>>,
}
impl DynamicDispatchTransmitter for TransmitterUDP {
    #[tracing::instrument]
    fn send(&self, from: &SlabRef, memoref: MemoRef) {
        if let Some(memo) = memoref.get_memo_if_resident() {
            let packet = SerdePacket { to_slab_id: Some(self.slab_id.clone()),
                                       from_slab_id: from.0.slab_id,
                                       memo,
                                       peerlist: memoref.get_peerlist_for_peer(from, Some(&self.slab_id)) };

            // use util::serde::SerializeHelper;
            // let helper = SerializeHelper{ transmitter: self };
            // wrapper = SerializeWrapper<Packet>
            // let b = serde_json::to_vec(&packet).expect("serde_json::to_vec");

            if let Some(ref tx_channel) = *(self.tx_channel.lock().unwrap()) {
                tx_channel.send((self.address.clone(), packet)).unwrap();
            }
        }
    }
}

impl fmt::Debug for TransmitterUDP {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("TransmitterUDP").field("address", &self.address).finish()
    }
}
