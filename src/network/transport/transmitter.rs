
use std::sync::mpsc;
use super::*;
use memo::PeeringStatus;

/// A trait for transmitters to implement
pub trait DynamicDispatchTransmitter {
    /// Transmit a memo to this Transmitter's recipient
    fn send (&self, from: &SlabRef, memo: Memo);
}

enum TransmitterInternal {
    Local(Arc<mpsc::Sender<(SlabRef,PeeringStatus,Memo)>>),
    Dynamic(Box<DynamicDispatchTransmitter + Send + Sync>)
}

pub struct Transmitter {
    internal: TransmitterInternal
}

impl Transmitter {
    /// Create a new transmitter associated with a local slab.
    pub fn new_local( tx: Arc<mpsc::Sender<(SlabRef,PeeringStatus,Memo)>> ) -> Self {
        Self {
            internal: TransmitterInternal::Local( tx )
        }
    }
    /// Create a new transmitter capable of using any dynamic-dispatch transmitter.
    pub fn new(dyn: Box<DynamicDispatchTransmitter + Send + Sync>) -> Self {
        Self {
            internal: TransmitterInternal::Dynamic(dyn)
        }
    }
    /// Send a Memo over to the target of this transmitter
    pub fn send(&self, from: &SlabRef, memo: Memo) {
        use self::TransmitterInternal::*;
        match self.internal {
            Local(ref tx) => {
                //println!("CHANNEL SEND from {}, {:?}", from.slab_id, memo);
                // TODO - stop assuming that this is resident on the sending slab just because we're sending it
                // TODO - lose the stupid lock on the transmitter
                tx.send((from.clone(),PeeringStatus::Resident,memo)).expect("local transmitter send")
            }
            Dynamic(ref tx) => {
                tx.send(from,memo)
            }
        }
    }
}
