
pub struct MemorySlab(Arc<SlabInner>);

impl Deref for MemorySlab {
    type Target = MemorySlabInner;
    fn deref(&self) -> &MemorySlabInner {
        &*self.0
    }
}

pub struct MemorySlabInner {
    pub id: SlabId,
    pub my_ref: SlabRef,
    handle: SlabHandle,
    counters: SlabCounter,
    storage: nihdb::Store,

    memo_wait_channels: Mutex<HashMap<MemoId,Vec<mpsc::Sender<Memo>>>>,

    subject_subscriptions: Mutex<HashMap<SubjectId, Vec<futures::sync::mpsc::Sender<MemoRefHead>>>>,
    index_subscriptions: Mutex<Vec<futures::sync::mpsc::Sender<MemoRefHead>>>,
    memoref_dispatch_tx_channel: Option<Mutex<mpsc::Sender<MemoRef>>>,
    memoref_dispatch_thread: RwLock<Option<thread::JoinHandle<()>>>,
    peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    peering_remediation_queue: Mutex<Vec<MemoRef>>,

    peer_refs: RwLock<Vec<SlabRef>>,
    net: Network,
    pub dropping: bool
}

impl Slab for NIHDBSlab {
    fn get_handle (&self) -> SlabHandle {
        self.handle.clone()
    }
    fn get_ref (&self) -> SlabRef {
        self.my_ref.clone()
    }
    fn add_receiver (&self, mpsc::UnboundedReceiver<(SlabRequest,oneshot::Sender<SlabResponse>)>);
    fn put_slabref(&self, slab_id: SlabId, presence: &[SlabPresence] ) -> SlabRef;

    #[derive(Clone)]
}