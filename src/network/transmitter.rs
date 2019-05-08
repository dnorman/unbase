
use std::sync::mpsc;
use super::*;
use crate::slab::*;

/// A trait for transmitters to implement
pub trait DynamicDispatchTransmitter {
    /// Transmit a memo to this Transmitter's recipient
    fn send (&self, from: &SlabRef, memoref: MemoRef);
}

enum TransmitterInternal {
    Local(Mutex<mpsc::Sender<(SlabRef,MemoRef)>>),
    Dynamic(Box<DynamicDispatchTransmitter + Send + Sync>),
    Blackhole
}

#[derive(Debug)]
pub enum TransmitterArgs<'a>{
    Local(&'a Slab),
    Remote(&'a SlabId, &'a TransportAddress)
}
impl<'a> TransmitterArgs<'a>{
    pub fn get_slab_id (&self) -> SlabId {
        match self {
            &TransmitterArgs::Local(ref s)     => s.id.clone(),
            &TransmitterArgs::Remote(ref id,_) => *id.clone()
        }
    }
}


impl TransmitterInternal {
    pub fn kind (&self) -> &str {
        match self {
            &TransmitterInternal::Local(_)   => "Local",
            &TransmitterInternal::Dynamic(_) => "Dynamic",
            &TransmitterInternal::Blackhole  => "Blackhole"
        }
    }
}

pub struct Transmitter {
    to_slab_id: SlabId,
    internal: TransmitterInternal
}

impl Transmitter {
    /// Create a new transmitter associated with a local slab.
    pub fn new_local( to_slab_id: SlabId, tx: Mutex<mpsc::Sender<(SlabRef,MemoRef)>> ) -> Self {
        Self {
            to_slab_id: to_slab_id,
            internal: TransmitterInternal::Local( tx )
        }
    }
    pub fn new_blackhole(to_slab_id: SlabId) -> Self {
        Self {
            to_slab_id: to_slab_id,
            internal: TransmitterInternal::Blackhole
        }
    }
    /// Create a new transmitter capable of using any dynamic-dispatch transmitter.
    pub fn new(to_slab_id: SlabId, trans: Box<DynamicDispatchTransmitter + Send + Sync>) -> Self {
        Self {
            to_slab_id: to_slab_id,
            internal: TransmitterInternal::Dynamic(trans)
        }
    }
    /// Send a Memo over to the target of this transmitter
    pub fn send(&self, from: &SlabRef, memoref: MemoRef) {
        //println!("Transmitter({} to: {}).send(from: {}, {:?})", self.internal.kind(), self.to_slab_id, from.slab_id, memoref );
        let _ = self.internal.kind();
        let _ = self.to_slab_id;

        use self::TransmitterInternal::*;
        match self.internal {
            Local(ref tx) => {
                //println!("CHANNEL SEND from {}, {:?}", from.slab_id, memo);
                // TODO - stop assuming that this is resident on the sending slab just because we're sending it
                // TODO - lose the stupid lock on the transmitter
                tx.lock().unwrap().send((from.clone(),memoref)).expect("local transmitter send")
            }
            Dynamic(ref tx) => {
                tx.send(from,memoref)
            }
            Blackhole => {
                println!("WARNING! Transmitter Blackhole transmitter used. from {:?}, memoref {:?}", from, memoref );
            }
        }
    }
}

impl Drop for TransmitterInternal{
    fn drop(&mut self) {
        //println!("# TransmitterInternal().drop");
    }
}
