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
use crate::buffer::SlabPresenceBufElement;

/// # A reference to a Slab
///
/// The referenced slab may be resident within the same process or within a foreign process
/// Posessing a SlabRef does not confer ownership, or even imply locality. It does however provide us with a way to
/// refer to a slab abstractly, and a means of getting messages to it.
#[derive(Clone)]
pub struct SlabRef(pub Arc<SlabRefInner>);

pub struct SlabRefInner {
    pub slab_id:        SlabId,
    pub owning_slab_id: SlabId, // for assertions only?
    pub presence:       RwLock<Vec<SlabPresence>>,
    pub tx:             Mutex<Transmitter>,
    pub return_address: RwLock<TransportAddress>,
}

impl SlabRef {
    pub fn id(&self) -> &SlabId {
        &self.0.slab_id
    }

    #[tracing::instrument]
    pub fn send(&self, from: &SlabRef, memoref: &MemoRef) {
        let tx = self.0.tx.lock().unwrap();
        tx.send(from, memoref.clone());
    }

    pub fn get_return_address(&self) -> TransportAddress {
        self.0.return_address.read().unwrap().clone()
    }

    pub fn apply_presence(&self, presence: &Vec<SlabPresenceBufElement>, net: &Network) -> bool {
        // TODO - what about old presence information? Presumably SlabPresence should also be causal, no?
unimplemented!()

//        if *self.id() == self.0.owning_slab_id {
//            return false; // the slab manages presence for its self-ref separately
//        }
//        let mut list = self.0.presence.write().unwrap();
//        for p in list.iter_mut() {
//            if p == presence {
//                mem::replace(p, presence.clone()); // Update anticipated liftime
//                return false; // no real change here
//            }
//        }
//        list.push(presence.clone());
//        return true; // We did a thing

//            for p in presence.iter() {
////                assert!(*slab_id == p.slab_id, "presence slab_id does not match the provided slab_id");
//
//                let mut _maybe_slab = None;
//                let args = if p.address.is_local() {
//                    // playing silly games with borrow lifetimes.
//                    // TODO: make this less ugly
//                    _maybe_slab = self.net.get_slabhandle(p.slab_id);
//
//                    if let Some(ref slab) = _maybe_slab {
//                        TransmitterArgs::Local(slab)
//                    } else {
//                        continue;
//                    }
//                } else {
//                    TransmitterArgs::Remote(&p.slab_id, &p.address)
//                };
//                // Returns true if this presence is new to the slabref
//                // False if we've seen this presence already
//
//                if slabref.apply_presence(p) {
//                    let new_trans = self.net.get_transmitter(&args).expect("assert_slabref net.get_transmitter");
//                    let return_address = self.net.get_return_address(&p.address).expect("return address not found");
//
//                    *slabref.0.tx.lock().expect("tx.lock()") = new_trans;
//                    *slabref.0.return_address.write().expect("return_address write lock") = return_address;
//                }
//            }
//
//        true
    }

    pub fn get_presence_for_remote(&self, return_address: &TransportAddress) -> Vec<SlabPresence> {
        // If the slabref we are serializing is local, then construct a presence that refers to us
        if *self.id() == self.0.owning_slab_id {
            // TODO: This is wrong. We should be sending presence for more than just self-refs.
            //       I feel like we should be doing it for all local slabs which are reachabe through our transport?

            // TODO: This needs much more thought. My gut says that we shouldn't be taking in a transport address here,
            //       but should instead be managing our own presence.
            let my_presence = SlabPresence { slab_id:  self.id().clone(),
                                             address:  return_address.clone(),
                                             lifetime: SlabAnticipatedLifetime::Unknown, };

            vec![my_presence]
        } else {
            self.0.presence.read().unwrap().clone()
        }
    }

    pub fn compare(&self, other: &SlabRef) -> bool {
        // When comparing equality, we can skip the transmitter
        self.id() == other.id() && *self.0.presence.read().unwrap() == *other.0.presence.read().unwrap()
    }
}

impl fmt::Debug for SlabRef {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SlabRef")
           .field("owning_slab_id", &self.0.owning_slab_id)
           .field("slab_id", &self.id())
           .field("presence", &*self.0.presence.read().unwrap())
           .finish()
    }
}

impl Drop for SlabRefInner {
    fn drop(&mut self) {
        //
    }
}
