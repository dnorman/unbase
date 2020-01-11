
extern crate unbase;
extern crate serde;
extern crate serde_json;

/*

use serde::de::*;

use unbase::subject::Subject;
use unbase::memo::{Memo,PeeringStatus};
use unbase::slab::Slab;
use unbase::network::{Network,Packet};
use unbase::network::packet::serde::PacketSeed;
*/
//use serde_json;

/*
#[test]
fn serialize() {

    let net = unbase::Network::create_new_system();
    //let simulator = unbase::network::transport::Simulator::new();
    //net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let context_a = slab_a.create_context();

    let record = Subject::new_kv(&context_a, "animal_type","Cat").unwrap();



    let net2 = unbase::Network::new();
    //let simulator = unbase::network::transport::Simulator::new();
    //net.add_transport( Box::new(simulator.clone()) );
    let slab_b = unbase::Slab::new(&net);

    check_roundtrip(&record, &net2, &slab_b);

}

fn check_roundtrip(record: &Subject, net: &Network, slab: &Slab){

    let memo = &record.get_head().to_vec()[0].get_memo(slab).unwrap();

    let packet = Packet{
        to_slab_id: 1,
        from_slab_id: 0,
        from_slab_peering_status: MemoPeeringStatus::Resident,
        memo: memo.clone()
    };

    let encoded = serde_json::to_string(&packet).expect("serde_json::to_string");
    println!("{}", encoded );

    let decoded_packet : Packet;
    {
        let packet_seed : PacketSeed = PacketSeed{ net: &net };

        let mut deserializer = serde_json::Deserializer::from_str(&encoded);
        decoded_packet = packet_seed.deserialize(&mut deserializer).expect("packet_seed.deserialize");
    }
    let decoded_memo : Memo = decoded_packet.memo;

    assert_eq!(*memo, decoded_memo);
}
*/
