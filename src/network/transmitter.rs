use futures::prelude::*;
use futures::future;

use crate::slab;
use crate::slab::prelude::*;
use crate::network::transport::TransportAddress;
use crate::buffer::NetworkBuffer;
use crate::error::*;

/// A trait for transmitters to implement
pub trait DynamicDispatchTransmitter {
    /// Transmit a memo to this Transmitter's recipient
    fn send (&self, buf: NetworkBuffer) -> future::FutureResult<(), Error>;
}

enum TransmitterInternal {
    // TODO1: consider changing this to slab::storage::StorageRequester instead of LocalSlabHandle
    Local(LocalSlabHandle),
    Dynamic(Box<DynamicDispatchTransmitter>),
    // Consider DynamicSend(Box<DynamicDispatchTransmitter+Send>)
    Blackhole
}

#[derive(Debug)]
pub enum TransmitterArgs<'a>{
    Local(LocalSlabHandle),
    Remote(&'a SlabRef, &'a TransportAddress)
}
impl<'a> TransmitterArgs<'a>{
    pub fn get_slabref (&self) -> SlabRef {
        match self {
            &TransmitterArgs::Local(ref handle)     => handle.slabref.clone(),
            &TransmitterArgs::Remote(ref slabref,_) => (*slabref).clone(),
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
    to_slab_id: slab::SlabId,
    internal: TransmitterInternal
}

impl Transmitter {
    /// Create a new transmitter associated with a local slab.
    pub fn new_local( to_slabhandle: LocalSlabHandle ) -> Self {
        Self {
            to_slab_id: to_slabhandle.slabref.slab_id(),
            internal: TransmitterInternal::Local( to_slabhandle )
        }
    }
    pub fn new_blackhole(to_slabref: SlabRef) -> Self {
        Self {
            to_slab_id: to_slabref.slab_id(),
            internal: TransmitterInternal::Blackhole
        }
    }
    /// Create a new transmitter capable of using any dynamic-dispatch transmitter.
    pub fn new(to_slabref: SlabRef, trans: Box<DynamicDispatchTransmitter>) -> Self {
        Self {
            to_slab_id: to_slabref.slab_id(),
            internal: TransmitterInternal::Dynamic(trans)
        }
    }
    /// Send a Memo over to the target of this transmitter
    pub fn send(&self, buf: NetworkBuffer ) -> Box<Future<Item=(), Error=Error>> {
        //println!("Transmitter({} to: {}).send(from: {}, {:?})", self.internal.kind(), self.to_slab_id, from.slab_id, memoref );
        let _ = self.internal.kind();
        let _ = self.to_slab_id;

        use self::TransmitterInternal::*;
        match self.internal {
            Local(ref handle) => {
                // TODO: Implement.
                unimplemented!()
                // buf.extract( handle )
            }
            Dynamic(ref tx) => {
                Box::new( tx.send(buf) )
            }
            Blackhole => {
                println!("WARNING! Transmitter Blackhole transmitter used." );

                Box::new( future::result(Ok(())) )
            }
        }
    }
}

impl Drop for TransmitterInternal{
    fn drop(&mut self) {
        //println!("# TransmitterInternal().drop");
    }
}
