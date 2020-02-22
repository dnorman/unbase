pub mod serde;

use super::*;
use crate::network::{
    Transmitter,
    TransportAddress,
};

use crate::{
    buffer::SlabPresenceBufElement,
    head::Head,
};
use std::{
    fmt,
    sync::Arc,
};

/// # A reference to a Slab, and container for channels to send messages to that slab
///
/// The referenced slab could be anywhere - in the same process, or on the other side of the planet/solar system
/// Each slabref points to one slab, but it's also managed by a local slab because the channels might differ even within the same
/// process. Because of this, slabrefs created by one slab should not be directly used with other slabs.
#[derive(Clone)]
pub struct SlabRef(pub(crate) Arc<SlabRefInner>);

/// Compare only the pointers for SlabRefs during equality tests
impl std::cmp::PartialEq for SlabRef {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

pub(crate) struct SlabChannel {
    pub(crate) address: TransportAddress,
    pub(crate) return_address: TransportAddress,
    pub(crate) liveness: TransportLiveness,
    tx: Transmitter,
    pub(crate) latest_clock: Head,
    // TODO put some stats / backpressure here
}

pub(crate) struct SlabRefInner {
    pub(crate) slab_id:  SlabId,
    pub(crate) channels: RwLock<Vec<SlabChannel>>,
}

impl SlabRef {
    pub fn id(&self) -> &SlabId {
        &self.0.slab_id
    }

    #[tracing::instrument]
    pub fn send(&self, from: &SlabRef, memoref: &MemoRef) {
        let channels = self.0.channels.read().unwrap();
        for channel in channels.iter() {
            println!("TRAFFIC\t({}) {} -> {}", from, memoref, self);
            channel.tx.send(from, memoref.clone());
        }
    }

    pub fn get_return_address(&self) -> TransportAddress {
        let channels = self.0.channels.read().unwrap();
        // HACK - randomly picking a return address is wrong
        channels[0].return_address.clone()
    }

    pub fn channel_count(&self) -> usize {
        self.0.channels.read().unwrap().len()
    }

    pub fn apply_presence(&self, presence_bufs: &Vec<SlabPresenceBufElement>, net: &Network) -> bool {
        // TODO - what about old presence information? Presumably SlabPresence should also be causal, no?

        let mut channels = self.0.channels.write().unwrap();
        let mut applied = false;

        for presence_buf in presence_bufs.iter() {
            // look for a channel with the same address

            if let Some((i, _channel)) = channels.iter().enumerate().find(|(_, c)| c.address == c.address) {
                // If they're telling us they're going away, then remove the channel
                if let TransportLiveness::Unavailable = presence_buf.liveness {
                    channels.remove(i);
                    applied = true;
                }
            } else {
                if let TransportLiveness::Available = presence_buf.liveness {
                    if let Ok((tx, return_address)) = net.get_transmitter_and_return_addr(&self.0.slab_id, &presence_buf.address)
                    {
                        channels.push(SlabChannel { address: presence_buf.address.clone(),
                                                    return_address,
                                                    liveness: presence_buf.liveness.clone(),
                                                    tx,
                                                    latest_clock: Head::Null });

                        applied = true;
                    } else {
                        // TODO - presumably this means we need to establish a relay via another node
                    }
                }
            }
        }

        applied
    }
}
impl std::fmt::Display for SlabRef {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_fmt(format_args!("SlabRef:{}", &self.id()))
    }
}
impl fmt::Debug for SlabRef {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SlabRef")
           .field("slab_id", &self.id())
           .field("channels", &self.0.channels.read().unwrap())
           .finish()
    }
}
impl fmt::Debug for SlabChannel {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SlabChannel")
           .field("address", &self.address)
           .field("liveness", &self.liveness)
           .field("latest_clock", &self.latest_clock)
           .finish()
    }
}

impl Drop for SlabRefInner {
    fn drop(&mut self) {
        //
    }
}
