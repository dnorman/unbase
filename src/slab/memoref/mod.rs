pub mod serde;

use subject::SubjectId;
use slab;
use slab::prelude::*;
use memorefhead::MemoRefHead;
use error::Error;

use std::sync::{Arc,RwLock};
use std::fmt;
use core::ops::Deref;
use futures::prelude::*;
use futures::future;

#[derive(Clone)]
pub struct MemoRef(pub Arc<MemoRefInner>);

impl Deref for MemoRef {
    type Target = MemoRefInner;
    fn deref(&self) -> &MemoRefInner {
        &*self.0
    }
}

pub struct MemoRefInner {
    pub id:       MemoId,
    pub owning_slab_id: slab::SlabId, // TODO - rename and conditionalize with a macro
    pub subject_id: Option<SubjectId>,
    //TODO1 - remove this: pub peerlist: RwLock<MemoPeerList>,
    pub ptr:      RwLock<MemoRefPtr>,
}

pub enum MemoRefPtr {
    /// Memo is in memory now, right here in fact
    Resident(Arc<Memo>),
    /// Memo Is stored on the local slab which owns this memoref, you'll need to look it up
    Local,
    /// Is stored on a remote slab
    Remote
}

impl MemoRefPtr {
    pub fn to_peering_status (&self) -> MemoPeerStatus {
        match self {
            &MemoRefPtr::Resident(_) => MemoPeerStatus::Resident,
            &MemoRefPtr::Remote      => MemoPeerStatus::Participating
        }
    }
}

impl MemoRef {
    pub fn to_head (&self) -> MemoRefHead {
        match self.subject_id {
            None => MemoRefHead::Anonymous{
                head: vec![self.clone()]
            },
            Some(subject_id) => 
                MemoRefHead::Subject {
                    subject_id: subject_id,
                    head: vec![self.clone()]
                }
        }
    }
    pub fn is_resident(&self) -> bool {
        match *self.ptr.read().unwrap() {
            MemoRefPtr::Resident(_) => true,
            _                       => false
        }
    }
    pub fn get_memo_if_resident(&self) -> Option<Memo> {
        match *self.ptr.read().unwrap() {
            MemoRefPtr::Resident(ref memo) => Some(memo.clone()),
            _ => None
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
    pub fn get_memo (&self, slab: &LocalSlabHandle) -> Box<Future<Item=Memo, Error=Error>> {
//        println!("Slab({}).MemoRef({}).get_memo()", self.owning_slab_id, self.id );
        assert!(self.owning_slab_id == slab.id,"requesting slab does not match owning slab");

        match *self.ptr.read().unwrap(){
            MemoRefPtr::Resident(ref memo) => {
                Box::new(future::result(Ok(memo.clone())))
            }
            MemoRefPtr::Local | MemoRefPtr::Remote  => {
                slab.get_memo( self.id )
            }
        }

    }
    pub fn descends (&self, memoref: &MemoRef, slab: &LocalSlabHandle) -> Box<Future<Item=bool, Error=Error>> {
        assert!(self.owning_slab_id == slab.id);

        unimplemented!();
        // Box::new(self.get_memo( slab ).and_then(|memo| {
        //     memo.descends(memoref,slab)
        // }))
    }

}

impl fmt::Debug for MemoRef{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("MemoRef")
           .field("id", &self.id)
           .field("owning_slab_id", &self.owning_slab_id)
           .field("subject_id", &self.subject_id)
           .field("resident", &match *self.ptr.read().unwrap() {
               MemoRefPtr::Remote      => false,
               MemoRefPtr::Resident(_) => true
           })
           .finish()
    }
}

impl PartialEq for MemoRef {
    fn eq(&self, other: &MemoRef) -> bool {
        // TODO: handle the comparision of pre-hashed memos as well as hashed memos
        self.id == other.id
    }
}

impl Drop for MemoRefInner{
    fn drop(&mut self) {
        //println!("# MemoRefInner({}).drop", self.id);
    }
}
