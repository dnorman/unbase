use crate::{
    network::{
        transmitter::DynamicDispatchTransmitter,
        SlabRef,
        Transmitter,
        TransmitterArgs,
        Transport,
        TransportAddress,
    },
    slab::{
        MemoRef,
        SlabHandle,
    },
    Network,
};
use async_trait::async_trait;
use std::fmt;
use tracing::{
    debug,
    span,
    Level,
};

use crate::{
    error::Error,
    util::simulator::{
        Point3,
        SimEvent,
        Simulator,
    },
};

// TODO: determine how to account for execution time in a deterministic way
// suggest each operation be assigned a delay factor, such that some or all resultant events are deterministically
// delayed
pub struct MemoPayload {
    from_slabref: SlabRef,
    dest:         SlabHandle,
    memoref:      MemoRef,
}

#[async_trait]
impl SimEvent for MemoPayload {
    async fn deliver(self) {
        let span = span!(Level::TRACE, "MemoPayload Deliver");
        let _guard = span.enter();

        // Critically important that we not wait for follow-on activities to occur here.
        // We need to persist this payload to the slab, *maybe* a little light housekeeping, and then GTFO.
        // We can't wait for any communication to occur between this slab and other slabs, if for no other
        // reason that it takes _time_ passing to deliver any such messages, and time is frozen until this delivery
        // completes

        // QUESTION: how do we deterministically model execution time here such that memos which might be emitted as an
        // immediate consequence of this delivery could be a different, yet deterministic, instant than this?

        // NOTE: this separation of delivery handling from follow-on events also applies to network delivery, because
        // We don't want to queue unprocessed messages in the network code.

        // TODO: think about how backpressure interacts with selective hearing behaviors, and how intelligently relay
        // that backpressure to other nodes who are sending stuff

        debug!("localizing slabref {:?}", &self.from_slabref);
        let slabref = self.dest.agent.localize_slabref(&self.from_slabref);
        debug!("localizing memoref {:?}", &self.memoref);
        self.dest.agent.localize_memoref(&self.memoref, &slabref, true);
        debug!("done");
    }
}

impl fmt::Debug for MemoPayload {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("MemoPayload")
           .field("dest", &self.dest.my_ref)
           .field("memo", &self.memoref.id)
           .finish()
    }
}

impl Transport for Simulator<MemoPayload> {
    fn is_local(&self) -> bool {
        true
    }

    fn make_transmitter(&self, args: &TransmitterArgs) -> Option<Transmitter> {
        if let TransmitterArgs::Local(ref slab) = *args {
            let tx = SimulatorTransmitter { source_point: Point3 { x: 1000,
                                                                   y: 1000,
                                                                   z: 1000, }, /* TODO: move this - not appropriate
                                                                                * here */
                                            dest_point:   Point3 { x: 1000,
                                                                   y: 1000,
                                                                   z: 1000, },
                                            simulator:    (*self).clone(),
                                            dest:         (*slab).clone(), };
            Some(Transmitter::new(args.get_slab_id(), Box::new(tx)))
        } else {
            None
        }
    }

    fn bind_network(&self, _net: &Network) {}

    fn unbind_network(&self, _net: &Network) {}

    fn get_return_address(&self, address: &TransportAddress) -> Result<TransportAddress, Error> {
        if let TransportAddress::Local = *address {
            Ok(TransportAddress::Local)
        } else {
            Err(Error::BadAddress)
        }
    }
}

pub struct SimulatorTransmitter {
    pub source_point: Point3,
    pub dest_point:   Point3,
    pub simulator:    Simulator<MemoPayload>,
    pub dest:         SlabHandle,
}

impl DynamicDispatchTransmitter for SimulatorTransmitter {
    #[tracing::instrument]
    fn send(&self, from_slabref: &SlabRef, memoref: MemoRef) {
        let evt = MemoPayload { from_slabref: from_slabref.clone(),
                                dest: self.dest.clone(),
                                memoref };

        self.simulator.add_event(evt, &self.source_point, &self.dest_point);
    }
}

impl fmt::Debug for SimulatorTransmitter {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SimulatorTransmitter")
           .field("dest", &self.dest.my_ref)
           .finish()
    }
}
