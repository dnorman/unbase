use crate::slab::{Slab,prelude::*};
use crate::stream::NetworkBuffer;
use crate::network::{Network, WeakNetwork, TransportAddress};

pub (crate) trait BufferReceiver {
    fn receive(&self, buffer: NetworkBuffer, source_address: &TransportAddress );
}

pub (crate) struct NetworkReceiver {
    slabs: Vec<Slab>,
    net: WeakNetwork
}

impl NetworkReceiver{
    pub fn new ( net: &Network ) -> NetworkReceiver{
        NetworkReceiver{
            slabs: Vec::new(),
            net: net.weak()
        }
    }
}
//
//
//impl BufferReceiver for NetworkReceiver{
//    fn receive(&self, buffer: NetworkBuffer, source_address: &TransportAddress ) {
//        unimplemented!()
//    }
//}