use std::sync::atomic::AtomicU32;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicIsize, Ordering};

struct SlabCounter{
    last_memo_id: AtomicU32,
    last_subject_id: AtomicU32,
    memos_received: AtomicU64,
    memos_redundantly_received: AtomicU64,
}

impl SlabCounter{
    pub fn new () -> Self {
        SlabCounter{
            next_memo_id: 5001,
            next_subject_id: 9001,
            memos_received: 0,
            memos_redundantly_received: 0,
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
    pub fn increment_redundantly_received (&self) {
        self.memos_received.fetch_add(1, Ordering::SeqCst);
    }
}