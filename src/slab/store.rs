extern crate nihdb;
use slab::memoref::MemoRef;
use slab::memo::{Memo,MemoId};

use std::collections::HashMap;

pub trait SlabStore {
    fn insert_memoref (&self, MemoRef);
    fn fetch_memoref  (&self, MemoId) -> Option<MemoRef>;
    fn insert_memo    (&self, Memo);
    fn fetch_memo     (&self, MemoId) -> Option<Memo>;
    //TODO implement purge
}

pub struct NIHDB {
    dir: String,
    store: nihdb::Store,
}

// TODO: think about remotizing memos - should we wait until the actual flush is done?
// TODO: defer serialization until storage is to be flushed?
impl SlabStore for NIHDB {
    fn insert_memoref (&self, memoref: MemoRef) {
        unimplemented!()
    }
    fn fetch_memoref  (&self, memo_id: MemoId) -> Option<MemoRef>{
        unimplemented!()
    }
    fn insert_memo    (&self, memo: Memo) {
        unimplemented!()
    }
    fn fetch_memo     (&self, memo_id: MemoId) -> Option<Memo> {
        unimplemented!()
    }
}

pub struct Memory {
    memorefs_by_id: HashMap<MemoId,MemoRef>,
    memos_by_id:    HashMap<MemoId,Memo>,
}

impl SlabStore for Memory {
    fn insert_memoref (&self, memoref: MemoRef) {
        unimplemented!()
    }
    fn fetch_memoref  (&self, memo_id: MemoId) -> Option<MemoRef>{
        unimplemented!()
    }
    fn insert_memo    (&self, memo: Memo) {
        unimplemented!()
    }
    fn fetch_memo     (&self, memo_id: MemoId) -> Option<Memo> {
        unimplemented!()
    }
}