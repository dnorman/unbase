pub mod memo;
pub mod slabref;
pub mod memoref;
mod memohandling;

pub use self::slabref::{SlabRef,SlabRefInner,SlabPresence,SlabAnticipatedLifetime};
pub use self::memoref::MemoRef;
pub use self::memo::Memo;

use self::memo::*;
use network::transport::{TransportAddress,TransmitterArgs};

use std::fmt;
use std::sync::mpsc;
use std::sync::mpsc::channel;
use std::sync::{Arc,Mutex,Weak};
use std::sync::atomic;
use std::collections::HashMap;
use std::collections::hash_map::Entry;

use network::Network;
use subject::SubjectId;
use memorefhead::*;
use context::{Context,WeakContext};


/* Initial plan:
 * Initially use Mutex-managed internal struct to manage slab storage
 * TODO: refactor to use a lock-free hashmap or similar
 */

pub type SlabId = u32;

#[derive(Clone)]
pub struct Slab {
    pub id: SlabId,
    inner: Arc<SlabInner>
}

struct SlabShared{
    pub id: SlabId,
    memorefs_by_id: HashMap<MemoId,MemoRef>,
    memo_wait_channels: HashMap<MemoId,Vec<mpsc::Sender<Memo>>>,
    subject_subscriptions: HashMap<SubjectId, Vec<WeakContext>>,
    memos_received: u64,
    memos_redundantly_received: u64,

    my_slab: Option<WeakSlab>,
    my_ref: Option<SlabRef>,
    peer_refs: Vec<SlabRef>,
    net: Network
}
struct SlabCounters{
    last_memo_id: u32,
    last_subject_id: u32,
}
struct SlabInner {
    pub id: SlabId,
    my_ref: Mutex<Option<SlabRef>>,
    shared: Mutex<SlabShared>,
    counters: Mutex<SlabCounters>
}

#[derive(Clone)]
pub struct WeakSlab{
    pub id: u32,
    inner: Weak<SlabInner>
}

#[derive(Debug)]
pub enum MemoOrigin<'a>{
    SameSlab,
    OtherSlab(&'a SlabRef, MemoPeeringStatus)
    // TODO: consider bifurcation into OtherSlabTrusted, OtherSlabUntrusted
    //       in cases where we want to reduce computational complexity by foregoing verification
}

impl Slab {
    pub fn new(net: &Network) -> Slab {
        let slab_id = net.generate_slab_id();

        let shared = SlabShared {
            id: slab_id,
            memorefs_by_id:        HashMap::new(),
            memo_wait_channels:    HashMap::new(),
            subject_subscriptions: HashMap::new(),
            memos_received: 0,
            memos_redundantly_received: 0,
            my_slab: None,
            my_ref: None,
            peer_refs: Vec::new(),
            net: net.clone()
        };

        let me = Slab {
            id: slab_id,
            inner: Arc::new(SlabInner {
                id: slab_id,
                my_ref: Mutex::new(None),
                shared: Mutex::new(shared),
                counters: Mutex::new(SlabCounters {
                    last_memo_id: 5000,
                    last_subject_id: 0,
                }),
            })
        };

        net.register_local_slab(&me);
/*
        {
            *(me.inner.my_ref.lock().unwrap()) = Some(my_ref.clone());
        }

        // not sure if there's a better way to do this, but I want to have a handy return address
*/
        {
            let mut shared = me.inner.shared.lock().unwrap();
            shared.my_slab = Some(me.weak());
            //shared.my_ref   = Some(my_ref);
        }

        net.conditionally_generate_root_index_seed(&me);

        me
    }
    pub fn weak (&self) -> WeakSlab {
        WeakSlab {
            id: self.id,
            inner: Arc::downgrade(&self.inner)
        }
    }
    pub fn get_ref(&self) -> SlabRef {
        // TODO: figure out how to get this to return a borrow, rather than cloning
        // TODO: determine a better way than a Mutex<Option<SlabRef>> it's dumb
        let my_ref : SlabRef = self.inner.my_ref.lock().unwrap().clone().unwrap();
        my_ref
    }
    pub fn get_root_index_seed (&self) -> Option<MemoRefHead> {
    println!("get_root_index_seed A" );
        let net;
        {
            let shared = self.inner.shared.lock().unwrap();
            println!("get_root_index_seed B" );
            net = shared.net.clone();
        }

        println!("get_root_index_seed C" );
        let seed = net.get_root_index_seed();

        println!("get_root_index_seed D" );

        seed
    }
    pub fn generate_subject_id(&self) -> SubjectId {
        let mut counters = self.inner.counters.lock().unwrap();
        counters.last_subject_id += 1;

        (self.id as u64).rotate_left(32) | counters.last_subject_id as u64
    }
    pub fn gen_memo_id (&self) -> MemoId {
        let mut counters = self.inner.counters.lock().unwrap();
        counters.last_memo_id += 1;

        (self.id as u64).rotate_left(32) | counters.last_memo_id as u64
    }
    // Convenience function for now, but may make sense to optimize this later
    pub fn put_memo (&self, memo_origin: &MemoOrigin, memo : Memo ) -> MemoRef {
        let mut memorefs = self.put_memos(memo_origin, vec![memo] );
        memorefs.pop().unwrap()
    }
    pub fn put_memos(&self, memo_origin: &MemoOrigin, memos : Vec<Memo> ) -> Vec<MemoRef> {
        if memos.len() == 0 { return Vec::new() }

        let mut shared = self.inner.shared.lock().unwrap();

        let memorefs = shared.put_memos(memo_origin, memos);

        memorefs

    }
    /*pub fn put_memo_from_other_local_slab(&self, from_slab_id: SlabId, memo: Memo ){
        let mut shared = self.inner.shared.lock().unwrap();
        shared.put_memo_from_other_local_slab(from_slab_id, memo);
    }*/
    pub fn count_of_memorefs_resident( &self ) -> u32 {
        let shared = self.inner.shared.lock().unwrap();
        shared.memorefs_by_id.len() as u32
    }
    pub fn count_of_memos_received( &self ) -> u64 {
        let shared = self.inner.shared.lock().unwrap();
        shared.memos_received
    }
    pub fn count_of_memos_reduntantly_received( &self ) -> u64 {
        let shared = self.inner.shared.lock().unwrap();
        shared.memos_redundantly_received
    }
    pub fn assert_slabref_from_local_slab(&self, peer_slab: &Slab ){
        let shared = self.inner.shared.lock().unwrap();
        shared.assert_slabref_from_local_slab( peer_slab );
    }
    pub fn peer_slab_count (&self) -> usize {
        let shared = self.inner.shared.lock().unwrap();

        println!("Slab({}).peer_slab_count = {}", self.id, shared.peer_refs.len() );
        shared.peer_refs.len()
    }
    pub fn create_context (&self) -> Context {
        Context::new(self)
    }
    pub fn subscribe_subject (&self, subject_id: u64, context: &Context) {
        let weakcontext : WeakContext = context.weak();

        let mut shared = self.inner.shared.lock().unwrap();

        if let Some(subs) = shared.subject_subscriptions.get_mut(&subject_id) {
            subs.push(weakcontext);
            return;
        }

        // Stoopid borrow checker
        shared.subject_subscriptions.insert(subject_id, vec![weakcontext]);
        return;
    }
    pub fn unsubscribe_subject (&self,  subject_id: u64, context: &Context ){
        println!("# Slab({}).unsubscribe_subject({})", self.id, subject_id);

        let mut shared = self.inner.shared.lock().unwrap();

        if let Some(subs) = shared.subject_subscriptions.get_mut(&subject_id) {
            let weak_context = context.weak();
            subs.retain(|c| {
                c.cmp(&weak_context)
            });
            return;
        }
    }
    pub fn memo_wait_channel (&self, memo_id: MemoId ) -> mpsc::Receiver<Memo> {

        let (tx, rx) = channel::<Memo>();
        let mut shared = self.inner.shared.lock().unwrap();

        match shared.memo_wait_channels.entry(memo_id) {
            Entry::Vacant(o)       => { o.insert( vec![tx] ); }
            Entry::Occupied(mut o) => { o.get_mut().push(tx); }
        };

        rx
    }
    pub fn remotize_memo_ids( &self, memo_ids: &[MemoId] ) {
        println!("# Slab({}).remotize_memo_ids({:?})", self.id, memo_ids);

        let mut memorefs : Vec<MemoRef> = Vec::with_capacity(memo_ids.len());

        {
            let shared = self.inner.shared.lock().unwrap();
            for memo_id in memo_ids.iter() {
                if let Some(memoref) = shared.memorefs_by_id.get(memo_id) {
                    memorefs.push( memoref.clone() )
                }
            }
        }

        for memoref in memorefs {
            memoref.remotize(&self)
        }
    }
}

impl WeakSlab {
    pub fn upgrade (&self) -> Option<Slab> {
        match self.inner.upgrade() {
            Some(i) => Some( Slab { id: self.id, inner: i } ),
            None    => None
        }
    }
}

impl SlabShared {
    pub fn assert_slabref_from_local_slab(&self, peer_slab: &Slab) -> SlabRef {

        let args = TransmitterArgs::Local(&peer_slab);
        let presence = SlabPresence{
            slab_id: peer_slab.id,
            address: TransportAddress::Local,
            lifetime: SlabAnticipatedLifetime::Unknown
        };

        self.assert_slabref(args, &presence)
    }
    pub fn assert_slabref_from_presence(&self, presence: &SlabPresence) -> Result<SlabRef,&str> {

        match presence.address {
            TransportAddress::Simulator  => {
                return Err("Invalid - Cannot create simulator slabref from presence")
            }
            TransportAddress::Local      => {
                return Err("Invalid - Cannot create local slabref from presence")
            }
            _ => { }
        };

        let args = TransmitterArgs::Remote( &presence.slab_id, &presence.address );

        Ok(self.assert_slabref( args, presence ))
    }
    fn assert_slabref(&self, args: TransmitterArgs, presence: &SlabPresence ) -> SlabRef {

        if let Some(slabref) = self.peer_refs.iter().find(|r| r.0.to_slab_id == presence.slab_id ) {
            if slabref.apply_presence(presence) {
                let new_trans = self.net.get_transmitter( args ).expect("new_from_slab net.get_transmitter");
                let return_address = self.net.get_return_address( &presence.address ).expect("return address not found");

                *(slabref.0.tx.lock().unwrap()) = new_trans;
                slabref.0.return_address.store(&mut return_address, atomic::Ordering::Relaxed);
            }
            return slabref.clone();
        }else{
            let tx = self.net.get_transmitter( args ).expect("new_from_slab net.get_transmitter");
            let return_address = self.net.get_return_address( &presence.address ).expect("return address not found");

            let inner = SlabRefInner {
                to_slab_id: presence.slab_id,
                owning_slab_id: self.id, // for assertions only?
                presence: Mutex::new(vec![presence.clone()]),
                tx: Mutex::new(tx),
                return_address: atomic::AtomicPtr::new(&mut return_address),
            };

            let slabref = SlabRef(Arc::new(inner));
            self.peer_refs.push(slabref);
            return slabref;
        };

    }
    pub fn get_my_slab <'a> (&self) -> Slab {
        if let Some(ref weak) = self.my_slab {
            if let Some(s) = weak.upgrade() {
                return s;
            }else{
                panic!("Weak self slab failed to upgrade - this should not happen");
            }
        }else{
            panic!("Called get_my_ref on unregistered slab");
        }
    }
    pub fn get_my_ref (&self) -> &SlabRef {
        if let Some(ref slabref) = self.my_ref {
            &slabref
        }else{
            panic!("Invalid state - Missing my_ref")
        }
    }
    pub fn get_root_index_seed (&self) -> Option<MemoRefHead> {
        self.net.get_root_index_seed()
    }
    pub fn put_memos<'a> (&mut self, memo_origin: &MemoOrigin, memos: Vec<Memo> ) -> Vec<MemoRef> {
        let mids : Vec<MemoId> = memos.iter().map(|x| -> MemoId{ x.id }).collect();
        println!("# SlabShared({}).put_memos({:?},{:?})", self.id, memo_origin, mids );

        // TODO: Evaluate more efficient ways to group these memos by subject
        let mut subject_updates : HashMap<SubjectId, MemoRefHead> = HashMap::new();
        let mut memorefs = Vec::with_capacity( memos.len() );

        // TODO: figure out how to appease the borrow checker without cloning this
        let my_slab = self.get_my_slab();
        let my_ref = my_slab.get_ref().clone();

        for memo in memos {

            self.memos_received += 1;
            println!("# \\ Memo Type {:?}", memo.inner.body );
            // Store the memoref - avoid creating duplicates
            let memoref = match self.memorefs_by_id.entry(memo.id) {
                Entry::Vacant(o)   => {
                    o.insert( MemoRef::new_from_memo(&memo) ).clone() // TODO: figure out how to prolong the borrow here & avoid clone
                }
                Entry::Occupied(o) => {
                    let mr = o.get();
                    if !mr.residentize(&my_slab, &memo) {
                        self.memos_redundantly_received += 1;
                    }
                    // TODO: consider whether all of the below code should be short circuited
                    //       in the case that we already have this memo resident

                    mr.clone()
                }
            };
println!("# SlabShared({}).put_memos(A)", self.id);
            match memo_origin {
                &MemoOrigin::SameSlab => {
                    //
                }
                &MemoOrigin::OtherSlab(origin_slabref,ref origin_peering_status) => {
    println!("# SlabShared({}).put_memos(B)", self.id);
                    self.check_memo_waiters(&memo);
        println!("# SlabShared({}).put_memos(C)", self.id);
                    self.handle_memo_from_other_slab(&memo, &memoref, &origin_slabref, &my_slab, &my_ref);

                    println!("# SlabShared({}).put_memos(D)", self.id);
                    memoref.update_peer(origin_slabref, origin_peering_status.clone());

                    println!("# SlabShared({}).put_memos(E)", self.id);
                    self.do_peering_for_memo(&memo, &memoref, &origin_slabref, &my_slab, &my_ref);

                    println!("# SlabShared({}).put_memos(F)", self.id);
                    if memo.subject_id > 0 {
                        let mut head = subject_updates.entry( memo.subject_id ).or_insert( MemoRefHead::new() );
                        head.apply_memoref(&memoref, &my_slab);
                    }

        println!("# SlabShared({}).put_memos(G)", self.id);
                }
            };

            //TODO: should we emit all memorefs, or just those with subject_ids?
            memorefs.push(memoref);
        }

        self.emit_memos(&memorefs);

        for (subject_id,head) in subject_updates {
            self.dispatch_subject_head(subject_id, &head);
        }

        memorefs
    }
    pub fn dispatch_subject_head (&self, subject_id: u64, head : &MemoRefHead){
        println!("# \t\\ SlabShared({}).dispatch_subject_head({}, {:?})", self.id, subject_id, head.memo_ids() );
        if let Some(subscribers) = self.subject_subscriptions.get( &subject_id ) {
            for weakcontext in subscribers {
                if let Some(context) = weakcontext.upgrade() {
                    context.apply_subject_head( subject_id, head );
                }

            }
        }
    }

    fn check_peering_target( &self, memo: &Memo ) -> u8 {
        if memo.does_peering() {
            5
        }else{
            // This is necessary to prevent memo routing loops for now, as
            // memoref.is_peered_with_slabref() obviously doesn't work for non-peered memos
            // something here should change when we switch to gossip/plumtree, but
            // I'm not sufficiently clear on that at the time of this writing
            0
        }
    }
/*    pub fn memo_durability_score( &self, _memo: &Memo ) -> u8 {
        // TODO: devise durability_score algo
        //       Should this number be inflated for memos we don't care about?
        //       Or should that be a separate signal?

        // Proposed factors:
        // Estimated number of copies in the network (my count = count of first order peers + their counts weighted by: uptime?)
        // Present diasporosity ( my diasporosity score = mean peer diasporosity scores weighted by what? )
        0
    }
*/
}

impl Drop for SlabInner {
    fn drop(&mut self) {
        println!("# Slab({}).drop", self.id);
        // TODO: Drop all observers? Or perhaps observers should drop the slab (weak ref directionality)
    }
}

impl fmt::Debug for Slab {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let shared = self.inner.shared.lock().unwrap();

        fmt.debug_struct("Slab")
            .field("slab_id", &self.id)
            .field("peer_refs", &shared.peer_refs)
            .field("memo_refs", &shared.memorefs_by_id)
            .finish()
    }
}