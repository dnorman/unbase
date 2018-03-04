use std::sync::atomic::{AtomicU32,Ordering};

pub struct SlabCounter{
    next_memo_id: AtomicU32,
    next_subject_id: AtomicU32,
    peer_slabs: AtomicU32,
    memos_received: AtomicU32,
    memos_redundantly_received: AtomicU32,
}

impl SlabCounter{
    pub fn new () -> Self {
        SlabCounter{
            next_memo_id:               AtomicU32::new(5001),
            next_subject_id:            AtomicU32::new(9001),
            peer_slabs:                 AtomicU32::new(0),
            memos_received:             AtomicU32::new(0),
            memos_redundantly_received: AtomicU32::new(0),
        }
    }
    pub fn next_memo_id (&self) -> u32 {
        self.next_memo_id.fetch_add(1, Ordering::SeqCst)
    }
    pub fn next_subject_id (&self) -> u32{
        self.next_subject_id.fetch_add(1, Ordering::SeqCst)
    }
    pub fn increment_memos_received (&self) {
        self.memos_received.fetch_add(1, Ordering::SeqCst);
    }
    pub fn increment_memos_redundantly_received (&self) {
        self.memos_received.fetch_add(1, Ordering::SeqCst);
    }
    pub fn get_memos_received (&self) -> u32 {
        self.memos_received.load(Ordering::SeqCst)
    }
    pub fn get_memos_redundantly_received (&self) -> u32 {
        self.memos_redundantly_received.load(Ordering::SeqCst)
    }
    pub fn set_peer_slabs (&self, slabs: u32) {
        self.peer_slabs.store(slabs, Ordering::SeqCst);
    }
    pub fn get_peer_slabs (&self) -> u32 {
        self.peer_slabs.load(Ordering::SeqCst)
    }
}