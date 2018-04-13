pub (crate) mod handle;
pub (crate) mod worker;

pub (crate) mod memory;

#[cfg(not(target_arch = "wasm32"))]
pub mod nihdb;

pub trait SlabStore {
    fn get_memo( &mut self, memoref: MemoRef, allow_remote: bool ) -> Box<Future<Item=Memo, Error=Error>>;
    fn put_memo (&mut self, memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef ) -> Box<Future<Item=MemoRef, Error=Error>>;
    fn put_memoref( &mut self, memo_id: MemoId, subject_id: SubjectId, peerset: MemoPeerSet) -> Box<Future<Item=MemoRef, Error=Error>>;
    fn send_memos ( &mut self, slabrefs: &[SlabRef], memorefs: &[MemoRef] ) -> Box<Future<Item=(), Error=Error>>;
    // TODO: Should be Vec<Option<SlabPresence>>.
    fn get_slab_presence (&mut self, slabrefs: Vec<SlabRef>) -> Box<Future<Item=Vec<SlabPresence>, Error=Error>>;
    fn put_slab_presence (&mut self, presence: SlabPresence ) -> Box<Future<Item=(), Error=Error>>;
    fn get_peerset (&mut self, memoref: Vec<MemoRef>, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerSet>, Error=Error>>;
}

pub enum LocalSlabRequest {
    GetMemo { memoref: MemoRef, allow_remote: bool },
    PutMemo { memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef },
    SendMemo { to_slabrefs: Vec<SlabRef>, memorefs: Vec<MemoRef> },
    RemotizeMemoIds{ memo_ids: Vec<MemoId> },
    GetSlabPresence { slabrefs: Vec<SlabRef> },
    PutSlabPresence { presence: SlabPresence },
    GetPeerSet { memorefs: Vec<MemoRef>, maybe_dest_slabref: Option<SlabRef> },
}

pub enum LocalSlabResponse {
    GetMemo( Memo ),
    PutMemo ( MemoRef ),
    SendMemo ( () ),
    RemotizeMemoIds( () ),
    GetSlabPresence( Vec<SlabPresence> ),
    PutSlabPresence( () ),
    GetPeerSet( Vec<MemoPeerSet> ),
}