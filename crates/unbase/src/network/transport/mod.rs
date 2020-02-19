//! Provides the framework and implementations for communications modules,
//! pluggable transports that allow connections between slabs. A `Transport` knows how to make
//! `Transmitter`s which can be used to send `Memo`s.

mod blackhole;
mod local_direct;
pub mod simulator;
mod udp;

pub use self::{
    blackhole::Blackhole,
    local_direct::LocalDirect,
    udp::*,
};
pub use super::transmitter::{
    DynamicDispatchTransmitter,
    Transmitter,
};

use crate::{
    error::Error,
    network::*,
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum TransportAddress {
    Blackhole,
    Simulator,
    Local,
    UDP(TransportAddressUDP),
    UDT,
    WebRTP,
    SCMP,
    Bluetooth,
    ShamefulTCP, // SHAME! SHAME! SHAME! ( yes, I _really_ want to discourage people from using TCP )
}

pub trait Transport {
    fn make_transmitter(&self, args: &TransmitterArgs) -> Option<Transmitter>;
    fn is_local(&self) -> bool;
    fn bind_network(&self, network: &Network);
    fn unbind_network(&self, network: &Network);
    fn get_return_address(&self, address: &TransportAddress) -> Result<TransportAddress, Error>;
}

impl TransportAddress {
    pub fn to_string(&self) -> String {
        use self::TransportAddress::*;
        match self {
            &Simulator => "Simulator".to_string(),
            &Local => "Local".to_string(),
            &UDP(ref a) => a.to_string(),
            _ => "UNKNOWN".to_string(),
        }
    }

    pub fn is_local(&self) -> bool {
        match self {
            &TransportAddress::Local => true,
            &TransportAddress::Simulator => true,
            _ => false,
        }
    }
}
