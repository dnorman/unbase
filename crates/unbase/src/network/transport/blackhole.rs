use super::*;

#[derive(Clone)]
pub struct Blackhole;

impl Blackhole {
    // TODO: Potentially, make this return an Arc of itself.
    pub fn new () -> Self{
        Blackhole
    }
}

impl Transport for Blackhole {
    fn is_local (&self) -> bool {
        true
    }
    fn make_transmitter (&self, args: &TransmitterArgs ) -> Option<Transmitter> {
        Some(Transmitter::new_blackhole(args.get_slab_id()))
    }

    fn bind_network(&self, _net: &Network) {}
    fn unbind_network(&self, _net: &Network) {}

    fn get_return_address  ( &self, _address: &TransportAddress ) -> Option<TransportAddress> {
        None
    }
}

impl Drop for Blackhole {
    fn drop (&mut self) {
        println!("# Blackhole.drop");
    }
}
