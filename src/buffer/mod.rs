
//! Buffer Structs, used for intermediate/compact representations
//!
//! Uses offsets inside a given Buffer to more compactly represent data rather than via duplication.
//! This is being processed in memory for now, but it's designed to be a streamed later

pub mod receiver;

use std::collections::HashMap;
use serde_json;
pub use serde::ser::Serialize;
pub use serde::de::Deserialize;

use error::*;
use network::TransportAddress;
use subject::SubjectId;
use slab::{self,prelude::*};

use self::receiver::BufferReceiver;

#[derive(Serialize,Deserialize)]
pub struct NetworkBuffer {
    segments: Vec<NetBufSegment>,

    #[serde(skip)]
    memoref_accum: Vec<MemoRef>,
    #[serde(skip)]
    slabref_accum: Vec<SlabRef>,
};

pub enum NetBuffSegment {
    Subject(SubjectId),
    SlabRef(SlabId),
    MemoRef(MemoRefBuffer),
    Memo(MemoBuffer),
    SlabPresence(SlabPresenceBuffer),
    MemoPeerState(MemoPeerStateBuffer),
}

    type SubjectOffset = u16;
    type SlabPresenceOffset = u16;
    type MemoRefOffset = u16;

    /// `MemoId`, offset of the Memo's subject id, list of slab presence offsets wherein this memo is purportedly present, list of slab presence offsets wherein this memo is peered but not present
    #[derive(Serialize
},Deserialize)]
struct MemoRefBuffer( MemoId, SubjectOffset );

#[derive(Serialize,Deserialize)]
pub struct MemoBuffer( MemoRefOffset, Vec<MemoRefOffset>, MemoBodyBuffer );

#[derive(Serialize,Deserialize)]
enum MemoBodyBuffer {
    SlabPresence{ p: SlabPresenceBuffer, r: Vec<MemoRefOffset> }, // TODO: split out root_index_seed conveyance to another memobody type
    Edge(EdgeSetBuffer),
    Edit(HashMap<String, String>),
    FullyMaterialized     { v: HashMap<String, String>, e: EdgeSetBuffer },
    PartiallyMaterialized { v: HashMap<String, String>, e: EdgeSetBuffer },
    Peering(MemoRefOffset,Vec<MemoPeerStateBuffer>),
    MemoRequest(Vec<MemoRefOffset>,Vec<SlabRefOffset>)
}

struct SlabRefBuffer {
    pub slab_id: SlabId,
}

#[derive(Serialize,Deserialize)]
type SlabPresenceBuffer = SlabPresence;

#[derive(Serialize,Deserialize)]
struct MemoPeerStateBuffer (
    slab: SlabPresenceOffset,
    status: MemoPeerStatus
)

pub struct EdgeSetBuffer (pub HashMap<RelationSlotId, Vec<MemoRefOffset>>);


impl NetworkBuffer{
    pub fn new(memos: Vec<Memo>, from_slab: &LocalSlabHandle ) -> Self {

        let mut netbuf = NetworkBuffer{
            segments: Vec::with_capacity(memos.len() * 4),
            slabref_accum = Vec::new(),
            memoref_accum = Vec::new(),
        };

        for memo in memos {
            netbuf.add_memo(memo);
        }

        from_slab.get

        netbuf
    }
    fn add_subject(&mut self, subject_id: SubjectId) -> SubjectOffset {
        match self.subject.iter().rposition(&subject_id) {
            Some(i) => i,
            None =>{
                self.subject.push(subject_id);
                self.subject.len() - 1
            }
        } as SubjectOffset
    }
    fn add_memo(&mut self, memo: Memo ) {
        // QUESTION: Should we dedup memos here?

        use MemoBody::*;
        let body = match memo.body {
            MemoBody::SlabPresence{ p, r } => {
                MemoBodyBuffer::SlabPresence {
                    p,
                    r: r.iter().map(|mr| self.add_memoref(mr.memo_id, self.add_subject(mr.subject_id))).collect(), // Assuming parents are the same subject_id as child
                }
            },
            MemoBody::Edge(edgeset) => {
                self.add_edge(edgeset)
            },
            MemoBody::Edit(hm) => {
                MemoBodyBuffer::Edit(hm)
            },
            MemoBody::FullyMaterialized{ v, e, t } => {
                MemoBodyBuffer::FullyMaterialized{ v, e: self.add_edge(e), t }
            },
            MemoBody::PartiallyMaterialized{ v, e, t } => {
                MemoBodyBuffer::PartiallyMaterialized{ v, e: self.add_edge(e), t }
            },
            MemoBody::Peering(memoref, peerset) => {
                MemoBodyBuffer::Peering(
                    self.add_memoref(mr.memo_id, self.add_subject(mr.subject_id)),
                    peerset.list.into_iter().map(|ps| self.add_peerstate(ps) )
                )
            },
            MemoBody::MemoRequest(memorefs, slabrefs) =>{
                MemoBodyBuffer::MemoRequest(
                    memorefs.into_iter().map(|mr| self.add_memoref(mr.memo_id, self.add_subject(mr.subject_id))).collect(),
                    slabrefs.into_iter().map(|r| self.add_slabref(r) ).collect()
                )
            }
        };

        let subj_offset = self.add_subject(memo.subject_id);

        let buf = MemoBuffer (
            self.add_memoref( memo_id, subj_offset ),
            memo.parents.iter().map(|mr| self.add_memoref( mr.memo_id, subj_offset) ).collect(), // Assuming parents are the same subject_id as child
            body
        );

        self.memo.push(buf);
    }
    fn add_edge(&mut self, e: EdgeSet) -> EdgeSetBuffer {
        let mut e = HashMap::new();
        for (slot_id,mrh) in edgeset {
            e.insert( slot_id, mrh.map(|mr| self.add_memoref(mr.memo_id, self.add_subject(mr.subject_id))).collect() );
        }
        MemoBodyBuffer::Edge(e)
    }
    fn add_memoref (&mut self, memo_id: MemoId, subj_offset: SubjectOffset) -> MemoRefOffset {
        match self.memo.iter().rposition(&memo_id) {
            Some(i) => i,
            None =>{
                self.memoref.push(MemoRefBuffer(
                    memo_id,
                    subj_offset,
                    vec![],
                ));
                self.memoref.len() - 1
            }
        } as MemoRefOffset
    }
    fn add_peerstate(&mut self, peerstate: MemoPeerState ) -> MemoPeerStateBuffer {
        MemoPeerStateBuffer(
            self.add_slabpresence(),
            peerstate.status
        )
    }
    fn add_slabpresence(&mut self, presence: SlabPresence) -> SlabPresenceOffset {
        match self.slabpresence.iter().rposition(presence) {
            Some(i) => i,
            None =>{
                self.slabpresence.push(presence);
                self.slabpresence.len() - 1
            }
        } as SlabPresenceOffset
    }
    pub fn from_slice(slice: &[u8]) -> Result<Self,Error> {
        serde_json::from_slice(slice).map_err(|e| Error::Serde(e))
    }
    pub fn to_vec(&self) -> Vec<u8> {
        unimplemented!()
    }
    fn extract_to( receiver: impl BufferReceiver ){
        unimplemented!()
    }
}