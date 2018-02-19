use super::*;

#[derive(Clone)]
pub struct LocalDirect {}

impl LocalDirect {
    // TODO: Potentially, make this return an Arc of itself.
    pub fn new () -> Self{
        LocalDirect {}
    }
}

impl Transport for LocalDirect {
    fn is_local (&self) -> bool {
        true
    }
    fn make_transmitter (&self, args: &TransmitterArgs ) -> Option<Transmitter> {
        if let &TransmitterArgs::Local(ref rcv_slab) = args {
            Some(Transmitter::new_local((*rcv_slab).clone()))
        }else{
            None
        }

    }

    fn bind_network(&mut self, _net: &Network) {}
    fn unbind_network(&mut self, _net: &Network) {}

    fn get_return_address  ( &self, address: &TransportAddress ) -> TransportAddress {
        if let TransportAddress::Local = *address {
            TransportAddress::Local
        }else{
            TransportAddress::Blackhole
        }
    }
}