// Note that a dynamic `import` statement here is required due to
// webpack/webpack#6615, but in theory `import { greet } from './pkg/hello_world';`
// will work here one day as well!
//const rust = import('./pkg/unbase');
//import { Network } from "./pkg/unbase";

void async function () {

    const {greet2, Network, Blackhole} = await import('./pkg/unbase');

     let net = Network.create_new_system();

     let blackhole = new Blackhole;
     net.add_blackhole_transport( blackhole );
// //    {
// //        let slab_a = unbase::Slab::new(&net);
// //        let _context_a = slab_a.create_context();
// //    }
//
// // Slabs should have been dropped by now
    console.log( net.get_local_slab_count() );
    greet2('Good','World!')

} ();