use wasm_bindgen::prelude::*;

use crate::network::*;

#[wasm_bindgen]
#[derive(Clone)]
pub struct Blackhole;

#[wasm_bindgen]
impl Blackhole {
    // TODO: Potentially, make this return an Arc of itself.
    #[wasm_bindgen(constructor)]
    pub fn new () -> Self{
        Blackhole
    }
}

impl Transport for Blackhole {
    fn is_local (&self) -> bool {
        true
    }
    fn make_transmitter (&self, args: &TransmitterArgs ) -> Option<Transmitter> {
        Some(Transmitter::new_blackhole(args.get_slabref()))
    }

    fn bind_network(&mut self, _net: &Network) {}
    fn unbind_network(&mut self, _net: &Network) {}

    fn get_return_address  ( &self, _address: &TransportAddress ) -> TransportAddress {
        TransportAddress::Blackhole
    }
}

impl Drop for Blackhole {
    fn drop (&mut self) {
        println!("# Blackhole.drop");
    }
}
