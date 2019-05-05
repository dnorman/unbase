use std::sync::Arc;
use std::collections::HashMap;
//use futures::prelude::*;
use futures::channel::{mpsc};
//use futures::future;
use nihdb;

use crate::slab::prelude::*;
use crate::slab::counter::SlabCounter;
use crate::network::Network;

pub struct NIHDBWorker {
    pub slabref: SlabRef,
    pub my_ref: SlabRef,
    handle: LocalSlabHandle,
    counters: Arc<SlabCounter>,
    storage: nihdb::Store,

    memo_wait_channels: HashMap<MemoId,Vec<mpsc::Sender<Memo>>>,

    // subject_subscriptions: Mutex<HashMap<SubjectId, Vec<futures::sync::mpsc::Sender<MemoRefHead>>>>,
    // index_subscriptions: Mutex<Vec<futures::sync::mpsc::Sender<MemoRefHead>>>,
    // memoref_dispatch_tx_channel: Option<Mutex<mpsc::Sender<MemoRef>>>,
    // memoref_dispatch_thread: RwLock<Option<thread::JoinHandle<()>>>,
    // peering_remediation_thread: RwLock<Option<thread::JoinHandle<()>>>,
    // peering_remediation_queue: Mutex<Vec<MemoRef>>,
    // peer_refs: RwLock<Vec<SlabRef>>,

    net: Network,
    pub dropping: bool
}