#![feature(proc_macro, conservative_impl_trait, generators)]
extern crate futures_await as futures;
use futures::prelude::*;

extern crate nihdb;
use slab::memoref::MemoRef;
use slab::memo::{Memo,MemoId};
use error::*;

use std::collections::HashMap;

pub trait SlabStore: Send+Sync {
    fn get_memo (&self, memo_id: &MemoId) -> Result<Option<Memo>, Error>;
    fn get_memoref (&self, memo_id: &MemoId) -> Result<Option<MemoRef>, Error>;
    fn fetch_memoref_stream (&self, Box<Stream<Item = &MemoId, Error = ()>>) -> Box<Stream<Item = Option<&MemoRef>, Error = Error>>;
    // fn insert_memoref (&self, MemoRef);
    // fn fetch_memoref  (&self, MemoId) -> Option<MemoRef>;
    // fn insert_memo    (&self, Memo);
    // fn fetch_memo     (&self, MemoId) -> Option<Memo>;
    //TODO implement purge
}

// pub struct NIHDB {
//     dir: String,
//     store: nihdb::Store,
// }

// TODO: think about remotizing memos - should we wait until the actual flush is done?
// TODO: defer serialization until storage is to be flushed?
// impl SlabStore for NIHDB {
    // fn insert_memoref (&self, memoref: MemoRef) {
    //     unimplemented!()
    // }
    // fn fetch_memoref  (&self, memo_id: MemoId) -> Option<MemoRef>{
    //     unimplemented!()
    // }
    // fn insert_memo    (&self, memo: Memo) {
    //     unimplemented!()
    // }
    // fn fetch_memo     (&self, memo_id: MemoId) -> Option<Memo> {
    //     unimplemented!()
    // }
// }

pub struct Memory {
    memorefs_by_id: HashMap<MemoId,MemoRef>,
    memos_by_id:    HashMap<MemoId,Memo>,
}

impl SlabStore for Memory {
    fn get_memo (&self, memo_id: &MemoId) -> Result<Option<Memo>, Error> {
        match self.memos_by_id.get(&memo_id){
            Some(m)  => Ok(Some(m.clone())),
            None     => Ok(None)
        }
    }
    fn get_memoref (&self, memo_id: &MemoId) -> Result<Option<MemoRef>, Error> {
        match self.memorefs_by_id.get(&memo_id){
            Some(mr) => Ok(Some(mr.clone())),
            None     => Ok(None)
        }
    }
    fn fetch_memoref_stream (&self, memo_ids: Box<Stream<Item = &MemoId, Error = ()>>) -> Box<Stream<Item = Option<&MemoRef>, Error = Error>> {
        unimplemented!()
        //memo_ids.map(|memo_id| self.memorefs_by_id.get(memo_id) )
    }
    // fn insert_memoref (&self, memoref: MemoRef) {
    //     unimplemented!()
    // }
    // fn fetch_memoref  (&self, memo_id: MemoId) -> Option<MemoRef>{
    //     unimplemented!()
    // }
    // fn insert_memo    (&self, memo: Memo) {
    //     unimplemented!()
    // }
    // fn fetch_memo     (&self, memo_id: MemoId) -> Option<Memo> {
    //     unimplemented!()
    // }
}

impl Memory {
    pub fn new () -> Self {
        Memory{
            memorefs_by_id: HashMap::new(),
            memos_by_id:    HashMap::new()
        }
    }
}