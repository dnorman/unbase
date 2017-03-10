//! Provides the framework and implementations for communications modules,
//! pluggable transports that allow connections between slabs. A `Transport` knows how to make
//! `Transmitter`s which can be used to send `Memo`s.

mod transmitter;
mod simulator;

pub use self::simulator::Simulator;
pub use self::transmitter::{Transmitter, DynamicDispatchTransmitter};

use network::SlabRef;
use slab::Slab;
use memo::Memo;

pub enum TransmitterArgs<'a>{
    Local(&'a Slab),
    Remote(&'a String)
}
pub use self::simulator::Simulator as Transport;
/*
pub trait Transport {
    fn make_transmitter(  &self, args: TransmitterArgs  ) -> Result<Transmitter,String>;
    fn is_local        (  &self ) -> bool;
}
*/
