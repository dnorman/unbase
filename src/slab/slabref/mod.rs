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

pub mod serde;

use super::*;
use crate::network::{TransportAddress,Transmitter};

use std::ops::Deref;
use std::mem;
use std::fmt;
use std::sync::{Arc,Mutex};

/// A reference to a Slab
///
/// The referenced slab may be resident locally or Remotely
#[derive(Clone)]
pub struct SlabRef(pub Arc<SlabRefInner>);
impl Deref for SlabRef {
    type Target = SlabRefInner;
    fn deref(&self) -> &SlabRefInner {
        &*self.0
    }
}
pub struct SlabRefInner {
    pub slab_id: SlabId,
    pub owning_slab_id: SlabId, // for assertions only?
    pub presence: RwLock<Vec<SlabPresence>>,
    pub tx: Mutex<Transmitter>,
    pub return_address: RwLock<TransportAddress>,
}

impl SlabRef{
    //pub fn new (to_slab_id: SlabId, owning_slab_id: SlabId, presence: Vec<Slab) -> SlabRef {
    //}
    pub fn send (&self, from: &SlabRef, memoref: &MemoRef ) {
        //println!("# Slab({}).SlabRef({}).send_memo({:?})", self.owning_slab_id, self.slab_id, memoref );

        self.tx.lock().unwrap().send(from, memoref.clone());
    }

    pub fn get_return_address(&self) -> TransportAddress {
        self.return_address.read().unwrap().clone()
    }
    pub fn apply_presence ( &self, presence: &SlabPresence ) -> bool {
        if self.slab_id == self.owning_slab_id{
            return false; // the slab manages presence for its self-ref separately
        }
        let mut list = self.presence.write().unwrap();
        for p in list.iter_mut(){
            if p == presence {
                mem::replace(p,presence.clone()); // Update anticipated liftime
                return false; // no real change here
            }
        }
        list.push(presence.clone());
        return true // We did a thing
    }
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
    pub fn clone_for_slab(&self, to_slab: &Slab ) -> SlabRef {
        // For now, we don't seem to care what slabref we're being cloned from, just which one we point to

        //println!("Slab({}).SlabRef({}).clone_for_slab({})", self.owning_slab_id, self.slab_id, to_slab.id );

        // IF this slabref points to the destination slab, then use to_sab.my_ref
        // because we know it exists already, and we're not allowed to assert a self-ref
        if self.slab_id == to_slab.id {
            to_slab.my_ref.clone()
        }else{
            //let address = &*self.return_address.read().unwrap();
            //let args = TransmitterArgs::Remote( &self.slab_id, address );
            to_slab.assert_slabref( self.slab_id, &*self.presence.read().unwrap() )
        }

    }
}

impl fmt::Debug for SlabRef {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SlabRef")
            .field("owning_slab_id", &self.owning_slab_id)
            .field("to_slab_id",     &self.slab_id)
            .field("presence",       &*self.presence.read().unwrap())
            .finish()
    }
}

impl Drop for SlabRefInner{
    fn drop(&mut self) {
        //println!("# SlabRefInner({},{}).drop",self.owning_slab_id, self.slab_id);
    }
}
