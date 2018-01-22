#![feature(proc_macro, conservative_impl_trait, generators)]
extern crate futures_await as futures;
use futures::prelude::*;

extern crate nihdb;
use slab::memoref::MemoRef;
use slab::memo::{Memo,MemoId};
use subject::{SubjectId};
use slab::common_structs::MemoPeerList;
use error::*;
use std::

use std::collections::HashMap;

pub trait SlabStore: Send+Sync {
    fn put_memo (&self, memo: &Memo) -> Result<(MemoRef,bool), Error>;
    fn assert_memoref( &self, memo_id: MemoId, subject_id: Option<SubjectId>, peerlist: MemoPeerList, maybe_memo: Option<Memo>) -> (MemoRef, bool);
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
    by_id: HashMap<MemoId,MemoRef>
}

impl Slab for Memory {
    /// remove the memo itself from storage, but not the reference to it. Returns Ok(true) if the memo was removed from the store, Ok(false) if it was not present, or Err(_) if there was a problem with removing it
    fn conditional_remove_memo (&self, memo_id) -> Result<bool,Error> {

        if let Some(memoref) = self.by_id.get(memo_id) {

            use MemoRefPtr::*;
            match memoref.ptr.write().unwrap() {
                ptr @ Resident(_) => {
                    if memoref.exceeds_min_durability_threshold() {
                        *ptr = MemoRefPtr::Remote;

                        return Ok(true);
                    }else{
                        return Err(Error::StorageOpDeclined(StorageOpDeclined::InsufficientPeering));
                    }
                },
                Local  => panic!("Sanity error. Memory Store does not implement MemoRefPtr::Local"),
                Remote => return Ok(false);
            }

        }else{
            Ok(false)
        }
    }
    fn put_memo (&self, memo: Memo) -> Result<(MemoRef, bool), Error> {

        let memo = Rc::new(Memo);
        self.memos_by_id.insert(memo.id, memo.clone());

        self.memorefs_by_id.entry(memo.id) {
            Entry::Vacant(o)   => {
                let mr = MemoRef(Arc::new(
                    MemoRefInner {
                        id: memo.id,
                        owning_slab_id: self.id,
                        subject_id: memo.subject_id,
                        peerlist: RwLock::new(vec![]),
                        ptr:      RwLock::new( MemoRefPtr::Resident(memo) )
                    }
                ));

                had_memoref = false;
                o.insert( mr ).clone()// TODO: figure out how to prolong the borrow here & avoid clone
            }
            Entry::Occupied(o) => {
                let mr = o.get();
                had_memoref = true;

                let mut ptr = mr.ptr.write().unwrap();
                if let MemoRefPtr::Remote = *ptr {
                    *ptr = MemoRefPtr::Resident(m)
                }
                mr.clone()
            }
        };

    }
    fn assert_memoref( &self, memo_id: MemoId, subject_id: Option<SubjectId>, peerlist: MemoPeerList, maybe_memo: Option<Memo>) -> (MemoRef, bool){

        match memo {
            Some(m) => {
                let memo = Rc::new(memo);

            }
            None => {
                
            }
        }

        let had_memoref;
        let memoref = match self.memorefs_by_id.entry(memo_id) {
            Entry::Vacant(o)   => {
                let mr = MemoRef(Arc::new(
                    MemoRefInner {
                        id: memo_id,
                        owning_slab_id: self.id,
                        subject_id: subject_id,
                        peerlist: RwLock::new(peerlist),
                        ptr:      RwLock::new(match memo {
                            Some(m) => {
                                assert!(self.id == m.owning_slab_id);
                                MemoRefPtr::Resident(m)
                            }
                            None    => MemoRefPtr::Remote
                        })
                    }
                ));

                had_memoref = false;
                o.insert( mr ).clone()// TODO: figure out how to prolong the borrow here & avoid clone
            }
            Entry::Occupied(o) => {
                let mr = o.get();
                had_memoref = true;
                if let Some(m) = memo {

                    let mut ptr = mr.ptr.write().unwrap();
                    if let MemoRefPtr::Remote = *ptr {
                        *ptr = MemoRefPtr::Resident(m)
                    }
                }
                mr.apply_peers( &peerlist );
                mr.clone()
            }
        };

        (memoref, had_memoref)
    }
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