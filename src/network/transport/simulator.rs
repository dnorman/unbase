
use std::fmt;
use std::io;
use std::thread;
use std::time;
use std::io::BufRead;
use futures::future;
//use futures::prelude::*;

use super::*;
use std::sync::{Arc,Mutex};
use itertools::partition;
//use network;
use buffer::NetworkBuffer;
use error::*;



// Minkowski stuff: Still ridiculous, but necessary for our purposes.
pub struct XYZPoint{
    pub x: i64,
    pub y: i64,
    pub z: i64
}
pub struct MinkowskiPoint {
    pub x: i64,
    pub y: i64,
    pub z: i64,
    pub t: u64
}
struct SimEvent {
    _source_point: MinkowskiPoint,
    dest_point:    MinkowskiPoint,
    dest_slab:     LocalSlabHandle,

    netbuf: NetworkBuffer,

}

impl SimEvent {
    pub fn deliver (self) {
        //println!("# SimEvent.deliver" );

//         println!("Simulator.deliver FROM {} TO {} -> {}({:?}): {:?} {:?} {:?}",
//             &self.from_slab_id,
//             &to_slab.id,
//             &self.memoref.id,
//             &self.memoref.subject_id,
//             &memo.body,
//             &memo.parents.memo_ids(),
//             &self.memoref.peerlist.read().unwrap().slab_ids()
//         );

        //self.buffer.deserialize_to_slabhandle(self.dest_slab, &from_address);

//        let from_slabref = dest_slab.put_slab_presence(self.from_presence);
//
//        self.dest_slab.put_memo(self.memo, self.peerset, from_slabref)
        unimplemented!()
    }
}
impl fmt::Debug for SimEvent{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SimEvent")
            .field("dest", &self.dest_slab.slabref )
            .field("t", &self.dest_point.t )
            .finish()
    }
}

#[derive(Clone)]
pub struct Simulator {
    shared: Arc<Mutex<SimulatorInternal>>,
    speed_of_light: u64,
    wait_clock: u64,
    run: Arc<Mutex<bool>>,
    //receiver: Option<BufferReceiver>,
}
struct SimulatorInternal {
    clock: u64,
    queue: Vec<SimEvent>,
}

impl Simulator {
    pub fn new() -> Self{
        Simulator {
            speed_of_light: 1, // 1 distance unit per time unit
            run: Arc::new(Mutex::new(true)),
            shared: Arc::new(Mutex::new(
                SimulatorInternal {
                    clock: 0,
                    queue: Vec::new()
                }
            )),
            wait_clock: 0,
        }
    }

    fn add_event(&self, event: SimEvent) {
        let mut shared = self.shared.lock().unwrap();
        shared.queue.push(event);
    }
    pub fn manual_time_step (&self) -> thread::JoinHandle<()> {
        let sim = self.clone();
        thread::spawn(move ||{
            println!("Simulator transport - Manual Timestep Enabled.\n\nPress ENTER to step\n\n" );
            let stdin = io::stdin();
            for _line in stdin.lock().lines() {
                sim.advance_clock(1);
            }
        })
    }
    pub fn metronome (&self, ms: u64) -> thread::JoinHandle<()> {
        let sim = self.clone();
        thread::spawn(move ||{
            println!("Simulator transport - Metronome Timestep Enabled (interval {}ms).\n\n", ms );
            loop {
                thread::sleep(time::Duration::from_millis(ms));
                let run = sim.run.lock().unwrap();

                if *run {
                    sim.advance_clock(1);
                }
            }
        })
    }
    pub fn pause(&self){
        let mut run = self.run.lock().unwrap();
        *run = false;
    }
    pub fn resume(&self) {
        let mut run = self.run.lock().unwrap();
        *run = true;
    }
    pub fn clear_wait(&mut self) {
        self.wait_clock = self.get_clock();
    }
    pub fn wait_ticks(&mut self, count: u64) {
        loop{
            thread::sleep(time::Duration::from_millis(10));
            let clock = self.get_clock();
            if clock >= self.wait_clock + count {
                self.wait_clock = clock;
                break;
            }
        }
    }
    pub fn wait_idle(&mut self) {
        loop{
            thread::sleep(time::Duration::from_millis(10));
            let shared = self.shared.lock().unwrap();

            if shared.queue.is_empty() {
                self.wait_clock = shared.clock;
                break;
            }
        }
    }
    pub fn get_clock(&self) -> u64 {
        self.shared.lock().unwrap().clock
    }
    pub fn advance_clock (&self, ticks: u64) {

        //println!("# Simulator.advance_clock({})", ticks);

        let t;
        let events : Vec<SimEvent>;
        {
            let mut shared = self.shared.lock().unwrap();
            shared.clock += ticks;
            t = shared.clock;

            let split_index = partition(&mut shared.queue, |evt| evt.dest_point.t >= t );

            events = shared.queue.drain(0..split_index).collect();
        }
        for event in events {
            event.deliver();
        }
    }
}

impl fmt::Debug for Simulator {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let shared = self.shared.lock().unwrap();
        fmt.debug_struct("Simulator")
            .field("queue", &shared.queue)
            .finish()
    }
}

impl Transport for Simulator {
    fn is_local (&self) -> bool {
        true
    }
    fn make_transmitter (&self, args: &TransmitterArgs ) -> Option<Transmitter> {
        if let &TransmitterArgs::Local(ref slab) = args {
            let tx = SimulatorTransmitter{
                source_point: XYZPoint{ x: 1000, y: 1000, z: 1000 }, // TODO: move this - not appropriate here
                dest_point: XYZPoint{ x: 1000, y: 1000, z: 1000 },
                simulator: self.clone(),
                dest: (*slab).clone(),
            };
            Some(Transmitter::new(args.get_slabref(), Box::new(tx)))
        }else{
            None
        }

    }
    fn bind_network(&mut self, _net: &Network) {
//        self.receiver = Some(BufferReceiver::new(net))
    }
    fn unbind_network(&mut self, _net: &Network) {
//        self.receiver = None;
    }
    fn get_return_address  ( &self, address: &TransportAddress ) -> TransportAddress {
        if let TransportAddress::Local = *address {
            TransportAddress::Local
        }else{
            TransportAddress::Blackhole
        }
    }
}


pub struct SimulatorTransmitter{
    pub source_point: XYZPoint,
    pub dest_point: XYZPoint,
    pub simulator: Simulator,
    pub dest: LocalSlabHandle
}

impl DynamicDispatchTransmitter for SimulatorTransmitter {
    fn send (&self, netbuf: NetworkBuffer) -> future::FutureResult<(), Error> {
        let ref q = self.source_point;
        let ref p = self.dest_point;

        let source_point = MinkowskiPoint {
            x: q.x,
            y: q.y,
            z: q.z,
            t: self.simulator.get_clock()
        };

        let distance = (( ((q.x - p.x)^2) + ((q.y - p.y)^2) + ((q.z - p.z)^2) ) as f64).sqrt();

        let dest_point = MinkowskiPoint {
            x: p.x,
            y: p.y,
            z: p.z,
            t: source_point.t + ( distance as u64 * self.simulator.speed_of_light ) + 1 // add 1 to ensure nothing is instant
        };

        let evt = SimEvent {
            _source_point: source_point,
            dest_point,
            from_slabref,

            dest_slab: self.dest.clone(),
            netbuf
        };
//
//        self.simulator.add_event( evt );
//        future::result(Ok(()))
        unimplemented!()
    }
}
