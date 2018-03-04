use subject::SubjectId;
use slab::{self, prelude::*};
use memorefhead::MemoRefHead;
use error::Error;

use std::fmt;
use futures::prelude::*;

#[derive(Clone, Eq, PartialOrd, Ord)]
pub struct MemoRef {
    pub memo_id:        MemoId,
    #[cfg(debug_assertions)]
    pub owning_slab_id: slab::SlabId, // TODO - rename and conditionalize with a macro
    pub subject_id:     SubjectId,
}

impl MemoRef {
    pub fn new (owning_slab_id: &slab::SlabId, memo_id: MemoId, subject_id: SubjectId) -> Self {
        MemoRef{
            memo_id,
            subject_id,

            #[cfg(debug_assertions)]
            owning_slab_id: owning_slab_id.clone()
        }
    }
    pub fn memo_id (&self) -> MemoId {
        self.memo_id.clone()
    }
    pub fn to_head (&self) -> MemoRefHead {
        if self.subject_id.is_anonymous() {
            MemoRefHead::Anonymous{
                head: vec![self.clone()]
            }
        }else{
            MemoRefHead::Subject {
                subject_id: self.subject_id,
                head: vec![self.clone()]
            }
        }
    }
    // pub fn is_resident(&self) -> bool {
    //     unimplemented!()
    //     // match *self.ptr.read().unwrap() {
    //     //     MemoRefPtr::Resident(_) => true,
    //     //     _                       => false
    //     // }
    // }
    // pub fn get_memo_if_resident(&self) -> Option<Memo> {
    //     unimplemented!()
    //     // match *self.ptr.read().unwrap() {
    //     //     MemoRefPtr::Resident(ref memo) => Some(memo.clone()),
    //     //     _ => None
    //     // }
    // }
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
    // CONSIDER REMOVAL
//     pub fn get_memo (&self, slab: &LocalSlabHandle) -> Box<Future<Item=Memo, Error=Error>> {
// //        println!("Slab({}).MemoRef({}).get_memo()", self.owning_slab_id, self.id );
//         assert!(self.owning_slab_id == slab.id,"requesting slab does not match owning slab");

//         match *self.ptr.read().unwrap(){
//             MemoRefPtr::Resident(ref memo) => {
//                 Box::new(future::result(Ok(memo.clone())))
//             }
//             MemoRefPtr::Local | MemoRefPtr::Remote  => {
//                 slab.get_memo( self.id )
//             }
//         }

//     }
    pub fn descends (&self, _memoref: &MemoRef, slab: &LocalSlabHandle) -> Box<Future<Item=bool, Error=Error>> {

        #[cfg(debug_assertions)]
        assert_eq!(self.owning_slab_id == slab.slab_id());

        unimplemented!();
        // Box::new(self.get_memo( slab ).and_then(|memo| {
        //     memo.descends(memoref,slab)
        // }))
    }
    // TODO: Remove _include_memo.
    pub fn clone_for_slab (&self, from_slab: &mut LocalSlabHandle, to_slab: &LocalSlabHandle, _include_memo: bool ) -> Self{
        #[cfg(debug_assertions)]
        assert_eq!(from_slab.slab_id, self.owning_slab_id,       "Cannot clone foreign MemoRef");
        //println!("Slab({}).Memoref.clone_for_slab({})", self.owning_slab_id, self.id);

        // Because our from_slabref is already owned by the destination slab, there is no need to do peerlist.clone_for_slab
        let _peerlist = from_slab.get_peerset(vec![self.clone()], Some(to_slab.slabref.clone()));
        //println!("Slab({}).Memoref.clone_for_slab({}) C -> {:?}", self.owning_slab_id, self.id, peerlist);

        unimplemented!()
        // // TODO - reduce the redundant work here. We're basically asserting the memoref twice
        // let memoref = to_slab.assert_memoref(
        //     self.id,
        //     self.subject_id,
        //     peerlist.clone(),
        //     match include_memo {
        //         true => match *self.ptr.read().unwrap() {
        //             MemoRefPtr::Resident(ref m) => Some(m.clone_for_slab(from_slabref, to_slab, &peerlist)),
        //             MemoRefPtr::Remote          => None
        //         },
        //         false => None
        //     }
        // ).0;


        // //println!("MemoRef.clone_for_slab({},{}) peerlist: {:?} -> MemoRef({:?})", from_slabref.slab_id, to_slab.id, &peerlist, &memoref );

        // memoref
    }

}

impl fmt::Debug for MemoRef{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("MemoRef")
           .field("id", &self.memo_id)
           .field("subject_id", &self.subject_id)
        //    .field("resident", &match *self.ptr.read().unwrap() {
        //        MemoRefPtr::Remote      => false,
        //        MemoRefPtr::Resident(_) => true
        //    })
           .finish()
    }
}

impl PartialEq for MemoRef {
    fn eq(&self, other: &MemoRef) -> bool {
        // TODO: handle the comparision of pre-hashed memos as well as hashed memos
        self.memo_id == other.memo_id
    }
}

// impl Drop for MemoRefInner{
//     fn drop(&mut self) {
//         //println!("# MemoRefInner({}).drop", self.id);
//     }
// }
