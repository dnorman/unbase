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

use network::{TransportAddress,Transmitter,TransmitterArgs};
use slab::prelude::*;

use std::ops::Deref;
use std::mem;
use std::fmt;
use std::sync::{Arc,Mutex,RwLock};

/// A reference to a Slab which may be local or remote
#[derive(Clone)]
pub struct SlabRef(pub Arc<SlabRefInner>);

impl Deref for SlabRef {
    type Target = SlabRefInner;
    fn deref(&self) -> &SlabRefInner {
        &*self.0
    }
}
struct SlabRefInner {
    pub slab_id: SlabId,
    pub owning_slab_id: SlabId, // for assertions only
    presence: RwLock<Vec<SlabPresence>>,
    tx: Mutex<Transmitter>,
    return_address: RwLock<TransportAddress>,
}

impl SlabRef{
    pub fn new ( slab_id: SlabId, owning_slab_id: SlabId, transmitter: Transmitter ) -> Self {
        let return_address = transmitter.get_return_address();
        
        let inner = SlabRefInner{
            slab_id: slab_id,
            owning_slab_id: owning_slab_id,
            presence: RwLock::new(Vec::new()),
            tx: Mutex::new(transmitter),
            return_address: RwLock::new( return_address ),
        };

        SlabRef(Arc::new(inner))
    }
}

impl SlabRefInner {
    //pub fn new (to_slab_id: SlabId, owning_slab_id: SlabId, presence: Vec<Slab) -> SlabRef {
    //}
    pub fn send (&self, from: &SlabRef, memoref: &MemoRef ) {
        //println!("# Slab({}).SlabRef({}).send_memo({:?})", self.owning_slab_id, self.slab_id, memoref );

        self.tx.lock().unwrap().send(from, memoref.clone());
    }

    pub fn get_return_address(&self) -> TransportAddress {
        self.return_address.read().unwrap().clone()
    }
    /// Apply a list of SlabPresence to this slabref
    pub fn apply_presence ( &self, new_presence_list: &[SlabPresence] ) -> bool {
        if self.slab_id == self.owning_slab_id{
            return false; // the slab manages presence for its self-ref separately
        }

        let mut presence_list = self.presence.write().unwrap();
        for new_presence in new_presence_list.iter(){
            assert!(self.slab_id == new_presence.slab_id, "presence slab_id does not match the provided slab_id");

            let mut maybe_slab = None;
            let args = if new_presence.address.is_local() {
                // playing silly games with borrow lifetimes.
                // TODO: make this less ugly
                maybe_slab = self.net.get_slab(new_presence.slab_id);

                if let Some(ref slab) = maybe_slab {
                    TransmitterArgs::Local(slab)
                }else{
                    continue;
                }
            }else{
                TransmitterArgs::Remote( &new_presence.slab_id, &new_presence.address )
            };
             // Returns true if this presence is new to the slabref
             // False if we've seen this presence already

            let mut found = false;

            for presence in presence_list.iter_mut(){
                if presence == new_presence {
                    mem::replace( new_presence, presence.clone()); // Update anticipated liftime
                    found = true;
                    break;
                }
            }

            if !found {
                presence_list.push( new_presence.clone() );

                let new_trans = self.net.get_transmitter( &args ).expect("put_slabref net.get_transmitter");
                let return_address = self.net.get_return_address( &new_presence.address ).expect("return address not found");

                *self.tx.lock().expect("tx.lock()") = new_trans;
                *self.return_address.write().expect("return_address write lock") = return_address;
            }
        }
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
    pub fn clone_for_slab(&self, to_slab: &SlabHandle ) -> SlabRef {
        // For now, we don't seem to care what slabref we're being cloned from, just which one we point to

        //println!("Slab({}).SlabRef({}).clone_for_slab({})", self.owning_slab_id, self.slab_id, to_slab.id );

        // IF this slabref points to the destination slab, then use to_sab.my_ref
        // because we know it exists already, and we're not allowed to assert a self-ref
        if self.slab_id == to_slab.id {
            to_slab.my_ref.clone()
        }else{
            //let address = &*self.return_address.read().unwrap();
            //let args = TransmitterArgs::Remote( &self.slab_id, address );
            to_slab.put_slabref( self.slab_id, &*self.presence.read().unwrap() )
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
