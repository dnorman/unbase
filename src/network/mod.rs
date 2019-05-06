use wasm_bindgen::prelude::*;

mod transmitter;
//
pub mod transport;

//use crate::slab;
//pub use crate::slab::prelude::*;
pub use self::transport::{Transport, TransportAddress};
//use crate::util::system_creator::SystemCreator;
pub use self::transmitter::{Transmitter, TransmitterArgs};

// TODO
type LocalSlabHandle = u64;

use std::ops::Deref;
use std::sync::{Arc, Weak, RwLock};
use std::fmt;
//use crate::memorefhead::MemoRefHead;

#[wasm_bindgen(module="network")]
#[derive(Clone)]
pub struct Network(Arc<NetworkInner>);

impl Deref for Network {
    type Target = NetworkInner;
    fn deref(&self) -> &NetworkInner {
        &*self.0
    }
}

pub struct NetworkInner {
    next_slab_id: RwLock<u32>,
    localslabhandles: RwLock<Vec<LocalSlabHandle>>,
//    transports: RwLock<Vec<Box<Transport + Send + Sync>>>,
//    root_index_seed: RwLock<Option<(MemoRefHead, LocalSlabHandle)>>,
    create_new_system: bool,
}

pub struct WeakNetwork(Weak<NetworkInner>);

#[wasm_bindgen]
impl Network {
    /// Network handle
    /// This represents your joining an existing unbase system.
    /// (In production, this is the one you want)
    #[wasm_bindgen(constructor)]
    pub fn new() -> Network {
        Self::new_inner(false)
    }
    /// In test cases, you want to create a wholly new unbase system.
    /// You should not be using this in production, except the *first* time ever for that system
    pub fn create_new_system() -> Network {
        Self::new_inner(true)
    }
    fn new_inner(create_new_system: bool) -> Network {
        let net = Network(Arc::new(NetworkInner {
            next_slab_id: RwLock::new(0),
            localslabhandles: RwLock::new(Vec::new()),
//            transports: RwLock::new(Vec::new()),
//            root_index_seed: RwLock::new(None),
            create_new_system: create_new_system,
        }));

//        let localdirect = self::transport::LocalDirect::new();
//        net.add_transport(Box::new(localdirect));

        net
    }

    // TODO: remove this when slab ids are randomly generated
//    pub fn hack_set_next_slab_id(&self, id: slab::SlabId) {
//        *self.next_slab_id.write().unwrap() = id;
//    }
    pub fn get_local_slab_count(&self) -> usize {
        self.localslabhandles.read().unwrap().len()
    }
}

impl Network{
    pub fn weak(&self) -> WeakNetwork {
        WeakNetwork(Arc::downgrade(&self.0))
    }

//    pub fn add_transport(&self, mut transport: Box<Transport + Send + Sync>) {
//        if transport.is_local() {
//            // Can only have one is_local transport at a time. Filter out any other local transports when adding this one
//            let mut transports = self.transports.write().unwrap();
//            if let Some(mut removed) = transports.iter().position(|t| t.is_local()).map(|e| transports.remove(e)) {
//                //println!("Unbinding local transport");
//                removed.unbind_network(self);
//            }
//        }
//
//        transport.bind_network(self);
//        self.transports.write().unwrap().push(transport);
//    }

    pub fn generate_slab_id(&self) -> u32 {
        let mut next_slab_id = self.next_slab_id.write().unwrap();
        let id = *next_slab_id;
        *next_slab_id += 1;

        id
    }
//    pub fn get_local_slab_handle_by_id(&self, slab_id: &slab::SlabId) -> Option<LocalSlabHandle> {
//        if let Some(handle) = self.localslabhandles.read().unwrap().iter().find(|s| s.slabref.slab_id == *slab_id) {
//            return Some(handle.clone());
//        }
//
//        None
//    }
//    pub fn get_local_slab_handle(&self, slabref: SlabRef) -> Option<LocalSlabHandle> {
//        if let Some(handle) = self.localslabhandles.read().unwrap().iter().find(|s| s.slabref == slabref) {
//            return Some(handle.clone());
//        }
//
//        None
//    }
//    pub (crate) fn get_representative_slab(&self) -> Option<LocalSlabHandle> {
//        for handle in self.localslabhandles.read().unwrap().iter() {
//            if handle.is_live() {
//                return Some(handle.clone());
//            }
//        }
//        None
//    }
//    pub fn get_all_local_slab_handles(&self) -> Vec<LocalSlabHandle> {
//        // TODO: convert this into a iter generator that automatically expunges missing slabs.
//        let mut res: Vec<LocalSlabHandle> = Vec::new();
//        // let mut missing : Vec<usize> = Vec::new();
//
//        for handle in self.localslabhandles.read().unwrap().iter() {
//            res.push( handle.clone() );
//            // match slab.upgrade() {
//            //     Some(s) => {
//            //         res.push(s);
//            //     }
//            //     None => {
//            //         // TODO: expunge freed slabs
//            //     }
//            // }
//        }
//
//        res
//    }
//    pub fn get_transmitter(&self, args: TransmitterArgs) -> Option<Transmitter> {
//        for transport in self.transports.read().unwrap().iter() {
//            if let Some(transmitter) = transport.make_transmitter(&args) {
//                return Some(transmitter);
//            }
//        }
//        None
//    }
//    pub fn get_return_address<'a>(&self, address: &TransportAddress) -> Vec<TransportAddress> {
//        let mut addresses = Vec::new();
//
//        for transport in self.transports.read().unwrap().iter() {
//            addresses.push( transport.get_return_address(address) )
//        }
//
//        addresses
//    }
//    pub fn register_local_slab(&self, mut new_slab: LocalSlabHandle) {
//        // println!("# Network.register_slab {:?}", new_slab );
//
//        {
//            self.localslabhandles.write().unwrap().insert(0, new_slab.clone());
//        }
//
//        for mut prev_slab in self.get_all_local_slab_handles() {
//            prev_slab.register_local_slabref(&new_slab);
//            new_slab.register_local_slabref(&prev_slab);
//        }
//    }
//    pub fn deregister_local_slab(&self, slabref: SlabRef) {
//        // Remove the deregistered slab so get_representative_slab doesn't return it
//        {
//            let mut slabhandles = self.localslabhandles.write().expect("slabs write lock");
//            if let Some(_) = slabhandles.iter()
//                .position(|s| s.slabref == slabref)
//                .map(|e| slabhandles.remove(e)) {
//                    // Does anyone care if we succeeded in removing the slabhandle from our list?
//                    // println!("Unbinding Slab {}", removed.id);
//                    // let _ = removed.slab_id;
//                    //removed.unbind_network(self);
//            }
//        }
//
//        // If the deregistered slab is the one that's holding the root_index_seed
//        // then we need to move it to a different slab
//
//        let mut maybe_seed = self.root_index_seed.write().expect("root_index_seed write lock");
//
//        let mut take= false;
//
//        if let Some((ref mut seed_mrh, ref mut seed_slabhandle)) = *maybe_seed {
//            if seed_slabhandle.slabref == slabref {
//                if let Some(new_slab) = self.get_representative_slab() {
//                    // Clone our old MRH under the newly selected slab
//                    *seed_mrh = seed_mrh.clone_for_slab(seed_slabhandle, &new_slab, false);
//                    *seed_slabhandle = new_slab;
//                } else {
//                    take = true;
//                }
//            }
//        }
//
//        if take {
//            maybe_seed.take();
//        }
//    }
//    pub fn get_root_index_seed(&self, slab: &LocalSlabHandle) -> MemoRefHead {
//
//        let mut root_index_seed = self.root_index_seed.write().expect("root_index_seed read lock");
//
//        match *root_index_seed {
//            Some((ref seed, ref mut from_slabhandle)) => {
//
//                if from_slabhandle.slabref == slab.slabref {
//                    // seed is resident on the requesting slab
//                    seed.clone()
//                } else {
//                    seed.clone_for_slab(from_slabhandle, slab, true)
//                }
//            }
//            None => MemoRefHead::Null,
//        }
//
//    }
    pub fn conditionally_generate_root_index_seed(&self, slab: &LocalSlabHandle) -> bool {
//        {
//            if let Some(_) = *self.root_index_seed.read().unwrap() {
//                return false;
//            }
//        }
//
        if self.create_new_system {
    // TODO
//            // I'm a new system, so I can do this!
//            let seed = SystemCreator::generate_root_index_seed(slab);
//            *self.root_index_seed.write().unwrap() = Some((seed.clone(), slab.clone()));
//            return true;
        }

        false
    }
//    /// When we receive a root_index_seed from a peer slab that's already attached to a system,
//    /// we need to apply it in order to "join" the same system
//    ///
//    /// TODO: how do we decide if we want to accept this?
//    ///       do we just take any system seed that is sent to us when unseeded?
//    ///       Probably good enough for Alpha, but obviously not good enough for Beta
//    pub fn apply_root_index_seed(&self,
//                                 _presence: &SlabPresence,
//                                 root_index_seed: &MemoRefHead,
//                                 resident_slabhandle: &LocalSlabHandle)
//                                 -> bool {
//
//        {
//            if let Some(_) = *self.root_index_seed.read().unwrap() {
//                // TODO: scrutinize the received root_index_seed to see if our existing seed descends it, or it descends ours
//                //       if neither is the case ( apply currently allows this ) then reject the root_index_seed and return false
//                //       this is use to determine if the SlabPresence should be blackholed or not
//
//                // let did_apply : bool = internals.root_index_seed.apply_disallow_diverse_root(  root_index_seed )
//                // did_apply
//
//                // IMPORTANT NOTE: we may be getting this root_index_seed from a different slab than the one that initialized it.
//                //                 it is imperative that all memorefs in the root_index_seed reside on the same local slabref
//                //                 so, it is important to undertake the necessary dilligence to clone them to that slab
//
//                return false;
//            }
//        }
//
//        *self.root_index_seed.write().unwrap() = Some((root_index_seed.clone(),
//                                                       resident_slabhandle.clone()));
//        true
//
//    }
}

impl fmt::Debug for Network {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Network")
            .field("next_slab_id", &self.next_slab_id.read().unwrap())
            .finish()
    }
}

// probably wasn't ever necessary, except as a way to debug
// impl Drop for NetworkInner {
// fn drop(&mut self) {
// println!("# > Dropping NetworkInternals");
//
// println!("# > Dropping NetworkInternals B");
// self.transports.clear();
//
// println!("# > Dropping NetworkInternals C");
// self.root_index_seed.take();
//
// println!("# > Dropping NetworkInternals D");
//
// }
// }
//

impl WeakNetwork {
    pub fn upgrade(&self) -> Option<Network> {
        match self.0.upgrade() {
            Some(i) => Some(Network(i)),
            None => None,
        }
    }
}
