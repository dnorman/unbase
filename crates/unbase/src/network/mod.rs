mod transmitter;

pub mod packet;
pub mod transport;

pub use self::{
    packet::SerdePacket,
    transmitter::{
        Transmitter,
        TransmitterArgs,
    },
    transport::{
        Transport,
        TransportAddress,
    },
};
pub use crate::slab::{
    agent::SlabAgent,
    SlabPresence,
    SlabRef,
    TransportLiveness,
};
use crate::util::system_creator::SystemCreator;

use crate::{
    error::Error,
    head::Head,
    slab::{
        SlabHandle,
        SlabId,
    },
};
use std::{
    fmt,
    ops::Deref,
    sync::{
        Arc,
        Mutex,
        RwLock,
        Weak,
    },
};

#[derive(Clone)]
pub struct Network(Arc<NetworkInner>);

impl Deref for Network {
    type Target = NetworkInner;

    fn deref(&self) -> &NetworkInner {
        &*self.0
    }
}

pub struct NetworkInner {
    next_slab_id:      RwLock<u32>,
    slabs:             RwLock<Vec<SlabHandle>>,
    transports:        RwLock<Vec<Box<dyn Transport + Send + Sync>>>,
    root_index_seed:   RwLock<Option<(Head, SlabRef)>>,
    create_new_system: bool,
}

pub struct WeakNetwork(Weak<NetworkInner>);

impl Network {
    /// Network handle
    /// This represents your joining an existing unbase system.
    /// (In production, this is the one you want)
    pub fn new() -> Network {
        Self::new_inner(false)
    }

    /// In test cases, you want to create a wholly new unbase system.
    /// You should not be using this in production, except the *first* time ever for that system
    pub fn create_new_system() -> Network {
        // TODO - we need a way to merge systems
        // Must this be done with the root of one system subordinating to the root of another?
        // Or perhaps we can allow systems to split AND merge?

        Self::new_inner(true)
    }

    fn new_inner(create_new_system: bool) -> Network {
        let net = Network(Arc::new(NetworkInner { next_slab_id: RwLock::new(0),
                                                  slabs: RwLock::new(Vec::new()),
                                                  transports: RwLock::new(Vec::new()),
                                                  root_index_seed: RwLock::new(None),
                                                  create_new_system }));

        let localdirect = self::transport::LocalDirect::new();
        net.add_transport(Box::new(localdirect));

        net
    }

    pub fn weak(&self) -> WeakNetwork {
        WeakNetwork(Arc::downgrade(&self.0))
    }

    pub fn add_transport(&self, transport: Box<dyn Transport + Send + Sync>) {
        if transport.is_local() {
            // Can only have one is_local transport at a time. Filter out any other local transports when adding this
            // one
            let mut transports = self.transports.write().unwrap();
            if let Some(removed) = transports.iter().position(|t| t.is_local()).map(|e| transports.remove(e)) {
                removed.unbind_network(self);
            }
        }

        transport.bind_network(self);
        self.transports.write().unwrap().push(transport);
    }

    pub fn generate_slab_id(&self) -> u32 {
        let mut next_slab_id = self.next_slab_id.write().unwrap();
        let id = *next_slab_id;
        *next_slab_id += 1;

        id
    }

    pub fn get_slabhandle(&self, slab_id: &SlabId) -> Result<SlabHandle, Error> {
        match self.slabs.read().unwrap().iter().find(|s| s.my_ref.0.slab_id == *slab_id) {
            Some(slabhandle) => {
                if slabhandle.is_running() {
                    return Ok((*slabhandle).clone());
                } else {
                    Err(Error::SlabOffline)
                }
                // TODO - scrub non-resident slabs
            },
            None => Err(Error::SlabNotFound),
        }
    }

    fn get_representative_slab(&self) -> Option<SlabHandle> {
        for slabhandle in self.slabs.read().unwrap().iter() {
            if slabhandle.is_running() {
                return Some((*slabhandle).clone());
            }

            // TODO - scrub non-resident slabs
        }
        return None;
    }

    pub fn get_all_local_slabs(&self) -> Vec<SlabHandle> {
        // TODO: convert this into a iter generator that automatically expunges missing slabs.
        let mut res: Vec<SlabHandle> = Vec::new();
        // let mut missing : Vec<usize> = Vec::new();

        for slabhandle in self.slabs.read().unwrap().iter() {
            if slabhandle.is_running() {
                res.push(slabhandle.clone());
            }
            // TODO - scrub non-resident slabs
        }

        res
    }

    pub fn get_transmitter_and_return_addr(&self, presence: &SlabPresence) -> Result<(Transmitter, TransportAddress), Error> {
        let args = match presence.address {
            TransportAddress::Simulator => TransmitterArgs::Simulator(&self.get_slabhandle(&presence.slab_id)?),
            TransportAddress::Local => TransmitterArgs::Local(&self.get_slabhandle(&presence.slab_id)?),
            _ => TransmitterArgs::Remote(&presence.slab_id, &presence.address),
        };

        for transport in self.transports.read().unwrap().iter() {
            if let Some(transmitter) = transport.make_transmitter(&args) {
                let ret = transport.get_return_address(&presence.address)?;

                return Ok((transmitter, ret));
            }
        }
        Err(Error::TransmitterNotFound)
    }

    //    pub fn get_return_address<'a>(&self, address: &TransportAddress) -> Result<TransportAddress, Error> {
    //        for transport in self.transports.read().unwrap().iter() {
    //            if let Some(return_address) = transport.get_return_address(address) {
    //                return Ok(return_address);
    //            }
    //        }
    //        Err(Error::AddressNotFound)
    //    }

    #[tracing::instrument]
    pub fn register_local_slab(&self, new_slab: SlabHandle) -> Result<(), Error> {
        // Question: does this have to be done first?
        {
            self.slabs.write().unwrap().insert(0, new_slab.clone());
        }

        for prev_slab in self.get_all_local_slabs() {
            prev_slab.slabref_from_local_slab(&new_slab)?;
            new_slab.slabref_from_local_slab(&prev_slab)?;
        }
        Ok(())
    }

    #[tracing::instrument]
    pub fn deregister_local_slab(&self, slabref: &SlabRef) {
        //        // Remove the deregistered slab so get_representative_slab doesn't return it
        {
            let mut slabs = self.slabs.write().expect("slabs write lock");
            if let Some(removed) = slabs.iter().position(|s| s.my_ref == *slabref).map(|e| slabs.remove(e)) {
                // debug!("Unbinding Slab {}", removed.id);
                //                let _ = removed.my_ref.slab_id;
                // removed.unbind_network(self);
            }
        }

        // If the deregistered slab is the one that's holding the root_index_seed
        // then we need to move it to a different slab

        let mut root_index_seed = self.root_index_seed.write().expect("root_index_seed write lock");

        if let Some(ref mut r) = *root_index_seed {
            if r.1 == *slabref {
                if let Some(new_slab) = self.get_representative_slab() {
                    let owned_slabref = new_slab.agent.localize_slabref(&r.1);
                    r.0 = new_slab.agent.localize_head(&r.0, &owned_slabref, false);
                    r.1 = new_slab.my_ref.clone();
                    return;
                }
            // don't return
            } else {
                return;
            }
        }

        // No slabs left
        root_index_seed.take();
    }

    pub fn get_root_index_seed(&self, slab: &SlabHandle) -> Head {
        let root_index_seed = { self.root_index_seed.read().expect("root_index_seed read lock").clone() };

        match root_index_seed {
            Some((ref seed, ref from_slabref)) => slab.agent.localize_head(seed, from_slabref, true),
            None => Head::Null,
        }
    }

    #[tracing::instrument]
    pub fn get_root_index_seed_for_agent(&self, agent: &SlabAgent) -> Head {
        let root_index_seed = self.root_index_seed.read().expect("root_index_seed read lock");

        match *root_index_seed {
            Some((ref seed, ref from_slabref)) => agent.localize_head(seed, from_slabref, true),
            None => Head::Null,
        }
    }

    pub fn conditionally_generate_root_index_seed(&self, slab: &SlabHandle) -> bool {
        {
            if let Some(_) = *self.root_index_seed.read().unwrap() {
                return false;
            }
        }

        if self.create_new_system {
            // I'm a new system, so I can do this!
            let seed = SystemCreator::generate_root_index_seed(slab);
            *self.root_index_seed.write().unwrap() = Some((seed.clone(), slab.my_ref.clone()));
            return true;
        }

        false
    }

    /// When we receive a root_index_seed from a peer slab that's already attached to a system,
    /// we need to apply it in order to "join" the same system
    ///
    /// TODO: how do we decide if we want to accept this?
    ///       do we just take any system seed that is sent to us when unseeded?
    ///       Probably good enough for Alpha, but obviously not good enough for Beta
    pub fn apply_root_index_seed(&self, _presence: &SlabPresence, root_index_seed: &Head, resident_slabref: &SlabRef) -> bool {
        {
            if let Some(_) = *self.root_index_seed.read().unwrap() {
                // TODO: scrutinize the received root_index_seed to see if our existing seed descends it, or it descends
                // ours       if neither is the case ( apply currently allows this ) then reject the
                // root_index_seed and return false       this is use to determine if the SlabPresence
                // should be blackholed or not

                // let did_apply : bool = internals.root_index_seed.apply_disallow_diverse_root(  root_index_seed )
                // did_apply

                // IMPORTANT NOTE: we may be getting this root_index_seed from a different slab than the one that
                // initialized it.                 it is imperative that all memorefs in the
                // root_index_seed reside on the same local slabref                 so, it is important
                // to undertake the necessary dilligence to clone them to that slab

                return false;
            }
        }

        *self.root_index_seed.write().unwrap() = Some((root_index_seed.clone(), resident_slabref.clone()));
        true
    }
}

impl fmt::Debug for Network {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Network")
           .field("next_slab_id", &self.next_slab_id.read().unwrap())
           .finish()
    }
}

impl WeakNetwork {
    pub fn upgrade(&self) -> Option<Network> {
        match self.0.upgrade() {
            Some(i) => Some(Network(i)),
            None => None,
        }
    }
}
