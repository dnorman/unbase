use super::*;
use subject::{Subject,SubjectType};

impl Deref for Slab {
    type Target = SlabInner;
    fn deref(&self) -> &SlabInner {
        &*self.0
    }
}

impl Slab {
    pub fn new(net: &Network) -> Slab {
        let slab_id = net.generate_slab_id();

        let my_ref_inner = SlabRefInner {
            slab_id: slab_id,
            owning_slab_id: slab_id, // I own my own ref to me, obviously
            presence: RwLock::new(vec![]), // this bit is just for show
            tx: Mutex::new(Transmitter::new_blackhole(slab_id)),
            return_address: RwLock::new(TransportAddress::Local),
        };

        let my_ref = SlabRef(Arc::new(my_ref_inner));
        // TODO: figure out how to reconcile this with the simulator
        let (memoref_dispatch_tx_channel, memoref_dispatch_rx_channel) = mpsc::channel::<MemoRef>();

        let inner = SlabInner {
            id: slab_id,
            memorefs_by_id:        RwLock::new(HashMap::new()),
            memo_wait_channels:    Mutex::new(HashMap::new()),
            subject_subscriptions: Mutex::new(HashMap::new()),

            counters: RwLock::new(SlabCounters {
                last_memo_id: 5000,
                last_subject_id: 9000,
                memos_received: 0,
                memos_redundantly_received: 0,
            }),

            memoref_dispatch_tx_channel: Some(Mutex::new(memoref_dispatch_tx_channel)),
            memoref_dispatch_thread: RwLock::new(None),

            my_ref: my_ref,
            peer_refs: RwLock::new(Vec::new()),
            net: net.clone(),
            dropping: false
        };

        let me = Slab(Arc::new(inner));
        net.register_local_slab(&me);

        let weak_self = me.weak();

        // TODO: this should really be a thread pool, or get_memo should be changed to be nonblocking somhow
        *me.memoref_dispatch_thread.write().unwrap() = Some(thread::spawn(move || {
            while let Ok(memoref) = memoref_dispatch_rx_channel.recv() {
                if let Some(slab) = weak_self.upgrade(){
                    slab.dispatch_memoref(memoref);
                }
            }
        }));

        net.conditionally_generate_root_index_seed(&me);

        me
    }
    pub fn weak (&self) -> WeakSlab {
        WeakSlab {
            id: self.id,
            inner: Arc::downgrade(&self.0)
        }
    }
    pub fn get_root_index_seed (&self) -> MemoRefHead {
        self.net.get_root_index_seed(self)
    }
    pub fn create_context (&self) -> Context {
        Context::new(self)
    }
    pub (crate) fn subscribe_subject (&self, _subject: &Subject) {
        //unimplemented!()
        // TODO3 - create a closure, need to sort out what thread is doing the applying. One per slab?
    }
    pub fn unsubscribe_subject (&self,  _subject_id: u64, _context: &Context ){
        //unimplemented!()
        // if let Some(subs) = self.subject_subscriptions.write().unwrap().get_mut(&subject_id) {
        //     let weak_context = context.ref();
        //     subs.retain(|c| {
        //         c.cmp(&weak_context)
        //     });
        //     return;
        // }
    }
    pub fn memo_wait_channel (&self, memo_id: MemoId ) -> mpsc::Receiver<Memo> {
        let (tx, rx) = channel::<Memo>();

        match self.memo_wait_channels.lock().unwrap().entry(memo_id) {
            Entry::Vacant(o)       => { o.insert( vec![tx] ); }
            Entry::Occupied(mut o) => { o.get_mut().push(tx); }
        };

        rx
    }
    pub fn generate_subject_id(&self, stype: SubjectType) -> SubjectId {
        let mut counters = self.counters.write().unwrap();
        counters.last_subject_id += 1;
        let id = (self.id as u64).rotate_left(32) | counters.last_subject_id as u64;
        SubjectId{ id, stype }
    }
}

impl WeakSlab {
    pub fn upgrade (&self) -> Option<Slab> {
        match self.inner.upgrade() {
            Some(i) => Some( Slab(i) ),
            None    => None
        }
    }
}
