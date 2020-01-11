//! Provides the framework and implementations for communications modules,
//! pluggable transports that allow connections between slabs. A `Transport` knows how to make
//! `Transmitter`s which can be used to send `Memo`s.

mod local_direct;
mod simulator;
mod udp;
mod blackhole;

pub use self::udp::*;
pub use self::simulator::Simulator;
pub use self::local_direct::LocalDirect;
pub use self::blackhole::Blackhole;
pub use super::transmitter::{Transmitter, DynamicDispatchTransmitter};

use network::*;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransportAddress{
    Blackhole,
    Simulator,
    Local,
    UDP(TransportAddressUDP),
    UDT,
    WebRTP,
    SCMP,
    Bluetooth,
    ShamefulTCP // SHAME! SHAME! SHAME! ( yes, I _really_ want to discourage people from using TCP )
}

pub trait Transport {
    fn make_transmitter(  &self, args: &TransmitterArgs  ) -> Option<Transmitter>;
    fn is_local        (  &self ) -> bool;
    fn bind_network    (  &self, &Network );
    fn unbind_network  (  &self, &Network );
    fn get_return_address  ( &self, &TransportAddress ) -> Option<TransportAddress>;
}

impl TransportAddress {
    pub fn to_string (&self) -> String {

        use self::TransportAddress::*;
        match self {
            &Simulator   => "Simulator".to_string(),
            &Local       => "Local".to_string(),
            &UDP(ref a)  => a.to_string(),
            _            => "UNKNOWN".to_string(),
        }
    }
    pub fn is_local (&self) -> bool {
        match self {
            &TransportAddress::Local      => true,
            &TransportAddress::Simulator  => true,
            _                             => false
        }
    }
}
