pub mod serde;

use super::*;
use crate::network::{
    Transmitter,
    TransportAddress,
};

use std::{
    fmt,
    mem,
    ops::Deref,
    sync::{
        Arc,
        Mutex,
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

impl Deref for SlabRef {
    type Target = SlabRefInner;

    fn deref(&self) -> &SlabRefInner {
        &*self.0
    }
}

pub struct SlabRefInner {
    pub slab_id:        SlabId,
    pub presence:       RwLock<Vec<SlabPresence>>,
    pub tx:             Mutex<Transmitter>,
    pub return_address: RwLock<TransportAddress>,
}

impl SlabRef {
    #[tracing::instrument]
    pub fn send(&self, from: &SlabRef, memoref: &MemoRef) {
        let tx = self.tx.lock().unwrap();
        tx.send(from, memoref.clone());
    }

    pub fn get_return_address(&self) -> TransportAddress {
        self.return_address.read().unwrap().clone()
    }

    pub fn apply_presence(&self, presence: &SlabPresence) -> bool {
        // TODO - what about old presence information? Presumably SlabPresence should also be causal, no?

        if self.slab_id == self.owning_slab_id {
            return false; // the slab manages presence for its self-ref separately
        }
        let mut list = self.presence.write().unwrap();
        for p in list.iter_mut() {
            if p == presence {
                mem::replace(p, presence.clone()); // Update anticipated liftime
                return false; // no real change here
            }
        }
        list.push(presence.clone());
        return true; // We did a thing
    }

    pub fn get_presence_for_remote(&self, return_address: &TransportAddress) -> Vec<SlabPresence> {
        // If the slabref we are serializing is local, then construct a presence that refers to us
        if self.slab_id == self.owning_slab_id {
            // TODO: This is wrong. We should be sending presence for more than just self-refs.
            //       I feel like we should be doing it for all local slabs which are reachabe through our transport?

            // TODO: This needs much more thought. My gut says that we shouldn't be taking in a transport address here,
            //       but should instead be managing our own presence.
            let my_presence = SlabPresence { slabref:  self.slab_id,
                                             address:  return_address.clone(),
                                             lifetime: SlabAnticipatedLifetime::Unknown, };

            vec![my_presence]
        } else {
            self.presence.read().unwrap().clone()
        }
    }

    pub fn compare(&self, other: &SlabRef) -> bool {
        // When comparing equality, we can skip the transmitter
        self.slab_id == other.slab_id && *self.presence.read().unwrap() == *other.presence.read().unwrap()
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
