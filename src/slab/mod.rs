mod common_structs;
mod memo;
mod slabref;
mod memoref;
mod store;
mod counter;
mod handle;

pub type SlabId = u32;

pub mod prelude {
    pub use slab::SlabId; // Intentionally omitting trait Slab and MemorySlab from here
    pub use slab::handle::SlabHandle;
    pub use slab::common_structs::*;
    pub use slab::slabref::{SlabRef,SlabRefInner};
    pub use slab::memoref::{MemoRef,MemoRefInner,MemoRefPtr};
    pub use slab::memo::{MemoId,Memo,MemoInner,MemoBody};
    pub use slab::memoref::serde as memoref_serde;
    pub use slab::memo::serde as memo_serde;
}

pub trait Slab {
    fn handle (&self) -> self::handle::SlabHandle;
    fn receive_memo_with_peerlist(&self, memo: self::memo::Memo, peerlist: self::common_structs::MemoPeerList, from_slabref: self::slabref::SlabRef ){

        let (memoref, had_memoref) = self.assert_memoref(memo.id, memo.subject_id, peerlist.clone(), Some(memo.clone()) );

        {
            let mut counters = self.counters.write().unwrap();
            counters.memos_received += 1;
            if had_memoref {
                counters.memos_redundantly_received += 1;
            }
        }
        //println!("Slab({}).reconstitute_memo({}) B -> {:?}", self.id, memo_id, memoref );


        self.consider_emit_memo(&memoref);

        if let Some(ref memo) = memoref.get_memo_if_resident() {

            self.check_memo_waiters(memo);
            //TODO1 - figure out eventual consistency index update behavior. Think fairly hard about blockchain fan-in / block-tree
            // NOTE: this might be a correct place to employ selective hearing. Highest liklihood if the subject is in any of our contexts,
            // otherwise 
            self.handle_memo_from_other_slab(memo, &memoref, &origin_slabref);
            self.do_peering(&memoref, &origin_slabref);

        }

        if let Some(ref tx_mutex) = self.memoref_dispatch_tx_channel {
            tx_mutex.lock().unwrap().send(memoref.clone()).unwrap()
        }


    }
}