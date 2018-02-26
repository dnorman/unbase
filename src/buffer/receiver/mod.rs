use slab::{self,prelude::*};
use buffer::NetworkBuffer;
use network::{Network, WeakNetwork, TransportAddress};

pub (crate) trait BufferReceiver {
    fn receive(&self, buffer: NetworkBuffer, source_address: &TransportAddress );
}

pub (crate) struct NetworkReceiver {
    slabs: Vec<LocalSlabHandle>,
    net: WeakNetwork
}

impl NetworkReceiver{
    pub fn new ( net: &Network ) -> NetworkReceiver{
        NetworkReceiver{
            slabs: Vec::new(),
            net: net.weak()
        }
    }
    pub fn get_local_slab_handle_by_id <'a> (&'a mut self, slab_id: &slab::SlabId) -> Option<&'a LocalSlabHandle> {
        match self.slabs.binary_search_by(|s| s.slab_id.cmp(slab_id) ){
            Ok(i) => {
                Some(&self.slabs[i])
            }
            Err(i) =>{
                if let Some(net) = self.net.upgrade() {
                    if let Some(slab) = net.get_local_slab_handle_by_id(slab_id) {
                        Some(self.slabs.insert(i, slab));
                    }
                }

                None
            }
        }
    }
    pub fn get_representative_slab<'a> (&'a mut self) -> Option<&'a LocalSlabHandle> {
        for handle in self.slabs.iter() {
            if handle.is_live() {
                return Some(&handle);
            }
        }
        if let Some(net) = self.net.upgrade() {
            if let Some(slab) = net.get_representative_slab() {
                match self.slabs.binary_search_by(|s| s.slab_id.cmp(&slab.slab_id) ){
                    Ok(i) => {
                        return Some(&self.slabs[i]); // really shouldn't ever hit this
                    }
                    Err(i) =>{
                        self.slabs.insert(i, slab);
                        return Some(&self.slabs[i]);
                    }
                }
            }
        }
        // TODO: Double check this logic -- just got it to compile
        None
    }
}


impl BufferReceiver for NetworkReceiver{
    fn receive(&self, buffer: NetworkBuffer, source_address: &TransportAddress ) {
        unimplemented!()
    }
}