pub mod serde;

use super::*;
use crate::network::{
    Transmitter,
    TransportAddress,
};
use core::ops::Deref;

use crate::{
    buffer::{
        BufferHelper,
        HeadBufElement,
        SlabBuf,
        SlabPresenceBufElement,
    },
    error::Error,
    head::Head,
    slab::state::SlabStateBufHelper,
};
use ::serde::{
    Deserialize,
    Serialize,
};
use std::{
    fmt,
    sync::{
        Arc,
        RwLock,
    },
};

/// # A reference to a Slab
///
/// The referenced slab may be resident within the same process or within a foreign process
/// Posessing a SlabRef does not confer ownership, or even imply locality. It does however provide us with a way to
/// refer to a slab abstractly, and a means of getting messages to it.
#[derive(Clone, Eq)]
pub struct SlabRef(pub Arc<SlabRefInner>);

/// Compare only the pointers for SlabRefs during equality tests
impl std::cmp::PartialEq for SlabRef {
    fn eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }
}

struct SlabChannel {
    addr:         TransportAddress,
    return_addr:  TransportAddress,
    liveness:     TransportLiveness,
    tx:           Transmitter,
    latest_clock: Head,
    // TODO put some stats / backpressure here
}

impl Deref for SlabRef {
    type Target = SlabRefInner;

    fn deref(&self) -> &SlabRefInner {
        &*self.0
    }
}

pub struct SlabRefInner {
    pub slab_id:  SlabId,
    pub channels: RwLock<Vec<SlabChannel>>,
}

impl SlabRef {
    #[tracing::instrument]
    pub fn send(&self, from: &SlabRef, memoref: &MemoRef) {
        let tx = self.tx.lock().unwrap();
        tx.send(from, memoref.clone());
    }

    pub fn get_return_address(&self) -> Result<TransportAddress, Error> {
        match self.0.channels.get(0) {
            Some(channel) => Ok(channel.return_addr.clone()),
            None => Err(Error::ChannelNotFound),
        }

        self.return_address.read().unwrap().clone()
    }

    pub(in crate::slab) fn apply_presence_only(&self, presence: &SlabPresence, net: &Network) -> Result<bool, Error> {
        // TODO - what about old presence information? Presumably SlabPresence should also be causal, no?
        let channels = self.0.channels.write().unwrap();

        // find a channel with exactly this address
        match channels.binary_search_by(|c| c.addr.cmp(presence.address)) {
            Ok(i) => {
                if let TransportLiveness::Unavailable = presence.liveness {
                    channels.remove(i);
                    Ok(true)
                } else {
                    Ok(false)
                }
            },
            Err(i) => {
                if presence.liveness.is_available() {
                    let (tx, return_addr) = net.get_transmitter_and_return_addr(&presence)?;

                    channels.insert(i,
                                    SlabChannel { addr: presence.address.clone(),
                                                  return_addr,
                                                  liveness: presence.liveness.clone(),
                                                  tx,
                                                  latest_clock: Head::Null });

                    Ok(true)
                } else {
                    Ok(false)
                }
            },
        }
    }

    pub(crate) fn apply_presence_and_save(&self, presence: &SlabPresence, agent: &SlabAgent) -> Result<bool, Error> {
        let applied = self.apply_presence_only(presence, &agent.net)?;
        if applied {
            let buf: SlabBuf<EntityId, MemoId> = self.to_buf(SlabStateBufHelper {});
            agent.state.put_slab(&presence.slab_id, &buf);
        }
        Ok(applied)
    }

    pub fn get_presence_for_remote(&self, return_address: &TransportAddress) -> Vec<SlabPresence> {
        // If the slabref we are serializing is local, then construct a presence that refers to us
        if self.slab_id == self.owning_slab_id {
            // TODO: This is wrong. We should be sending presence for more than just self-refs.
            //       I feel like we should be doing it for all local slabs which are reachabe through our transport?

            // TODO: This needs much more thought. My gut says that we shouldn't be taking in a transport address here,
            //       but should instead be managing our own presence.
            let my_presence = SlabPresence { slab_id:  self.slab_id,
                                             address:  return_address.clone(),
                                             liveness: TransportLiveness::Unknown, };

            vec![my_presence]
        } else {
            self.presence.read().unwrap().clone()
        }
    }

    pub fn compare(&self, other: &SlabRef) -> bool {
        // When comparing equality, we can skip the transmitter
        self.slab_id == other.slab_id && *self.presence.read().unwrap() == *other.presence.read().unwrap()
    }

    pub fn to_buf<E, M, S, H>(&self, helper: &H) -> SlabBuf<E, M>
        where E: Serialize + Deserialize,
              M: Serialize + Deserialize,
              S: Serialize + Deserialize,
              H: BufferHelper<EntityToken = E, MemoToken = M, SlabToken = S>
    {
        SlabBuf { presence: self.presence
                                .iter()
                                .map(|p| {
                                    SlabPresenceBufElement::<E, M> { address:      p.address,
                                                                     liveness:     p.liveness,
                                                                     latest_clock: HeadBufElement::<E, M>::Null, }
                                })
                                .collect(), }
    }
}

impl std::fmt::Display for SlabRef {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_fmt(format_args!("SlabRef:{}", &self.0.slab_id.base64()[..4]))
    }
}

impl fmt::Debug for SlabRef {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SlabRef")
           .field("owning_slab", &self.owning_slabref.slab_id)
           .field("slab_id", &self.slab_id)
           .field("presence", &*self.presence.read().unwrap())
           .finish()
    }
}

impl Drop for SlabRefInner {
    fn drop(&mut self) {
        //
    }
}
