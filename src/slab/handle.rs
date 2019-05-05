/*
    SlabRef intends to provide an abstraction for refering to a remote slab.
    Posessing a SlabRef does not confer ownership, or even imply locality.
    It does however provide us with a way to refer to a slab abstractly,
    and a means of getting messages to it.

    I labored a fair bit about whether this is materially different from
    the sender itself, but I think it is important, at least conceptually.
    Also, the internals of the sender could vary dramatically, whereas the
    SlabRef can continue to serve its purpose without material change.
*/

use crate::network::{TransportAddress,Transmitter,TransmitterArgs};
use crate::slab::prelude::*;

use std::ops::Deref;
use std::mem;
use std::fmt;
use std::sync::{Arc,Mutex,RwLock};

// struct SlabHandle {
//     pub slab_id: SlabId,
//     tx:       Transmitter,
//     return_address: TransportAddress,
// }

// impl SlabRef{
//     pub fn new ( slab_id: SlabId, owning_slab_id: SlabId, transmitter: Transmitter ) -> Self {
//         let return_address = transmitter.get_return_address();
        
//         let inner = SlabRefInner{
//             slab_id: slab_id,
//             owning_slab_id: owning_slab_id,
//             presence: RwLock::new(Vec::new()),
//             tx: Mutex::new(transmitter),
//             return_address: RwLock::new( return_address ),
//         };

//         SlabRef(Arc::new(inner))
//     }
// }

impl SlabRefInner {
    /// Apply a list of SlabPresence to this slabref

    pub fn get_presence_for_remote(&self, return_address: &TransportAddress) -> Vec<SlabPresence> {

        // If the slabref we are serializing is local, then construct a presence that refers to us
        if self.slab_id == self.owning_slab_id {
            // TODO: This is wrong. We should be sending presence for more than just self-refs.
            //       I feel like we should be doing it for all local slabs which are reachabe through our transport?

            // TODO: This needs much more thought. My gut says that we shouldn't be taking in a transport address here,
            //       but should instead be managing our own presence.
            let my_presence = SlabPresence{
                slab_id: self.slab_id,
                address: return_address.clone(),
                lifetime: SlabAnticipatedLifetime::Unknown
            };

            vec![my_presence]
        }else{
            self.presence.read().unwrap().clone()
        }
    }
    pub fn compare(&self, other: &SlabRef) -> bool {
        // When comparing equality, we can skip the transmitter
        self.slab_id == other.slab_id && *self.presence.read().unwrap() == *other.presence.read().unwrap()
    }
}
