struct MemoCarrier{
    memo:    Option<Memo>,
    memoref: Option<MemoRef>,
}


pub struct MemorySlabWorker {
    pub id: SlabId,
    counters: SlabCounter,
    storage: HashMap<MemoId,MemoCarrier>,

    memo_wait_channels: Mutex<HashMap<MemoId,Vec<mpsc::Sender<Memo>>>>, // TODO: HERE HERE HERE - convert to per thread wait channel senders?
    subject_subscriptions: Mutex<HashMap<SubjectId, Vec<futures::sync::mpsc::Sender<MemoRefHead>>>>,
    index_subscriptions: Mutex<Vec<futures::sync::mpsc::Sender<MemoRefHead>>>,
    peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    peering_remediation_queue: Mutex<Vec<MemoRef>>,

    pub my_ref: SlabRef,
    peer_refs: RwLock<Vec<SlabRef>>,
    net: Network,
    pub dropping: bool
}
