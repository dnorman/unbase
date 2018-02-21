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
pub struct NetworkBuffer{
    subject: Vec<SubjectId>,
    slabref: Vec<SlabRefBuffer>,
    slabpresence: Vec<SlabPresenceBuffer>,
    memoref: Vec<MemoRefBuffer>,
    memo: Vec<MemoBuffer>
}

type SubjectOffset = u16;
type SlabPresenceOffset = u16;
type MemoRefOffset = u16;

/// `MemoId`, offset of the Memo's subject id, list of slab presence offsets wherein this memo is purportedly present, list of slab presence offsets wherein this memo is peered but not present
#[derive(Serialize,Deserialize)]
struct MemoRefBuffer( MemoId, SubjectOffset, Vec<MemoPeerStateBuffer>);

#[derive(Serialize,Deserialize)]
pub struct MemoBuffer( MemoRefOffset, Vec<MemoRefOffset>, MemoBodyBuffer );

#[derive(Serialize,Deserialize)]
enum MemoBodyBuffer {
    SlabPresence{ p: SlabPresenceOffset, r: Vec<MemoRefOffset> }, // TODO: split out root_index_seed conveyance to another memobody type
    Edge(EdgeSetBuffer),
    Edit(HashMap<String, String>),
    FullyMaterialized     { v: HashMap<String, String>, e: EdgeSetBuffer },
    PartiallyMaterialized { v: HashMap<String, String>, e: EdgeSetBuffer },
    Peering(MemoRefOffset,Vec<MemoPeerStateBuffer>),
    MemoRequest(Vec<MemoRefOffset>,Vec<SlabPresenceOffset>)
}

struct SlabRefBuffer {
    pub slab_id: SlabId,
}
// TODO: detangle slabref vs slabpresence. We should always express a presence for a slabref.
// TODO: The only question is: should we represent slabref separately from slabpresence? Probably not
#[derive(Serialize,Deserialize)]
type SlabPresenceBuffer = SlabPresence;

#[derive(Serialize,Deserialize)]
struct MemoPeerStateBuffer {
    slab: SlabPresenceOffset,
    status: MemoPeerStatus
}

pub struct EdgeSetBuffer (pub HashMap<RelationSlotId, Vec<MemoRefOffset>>);


impl NetworkBuffer{
    pub fn single(memo: Memo, from_slab: &LocalSlabHandle ) -> Self {

        let mut netbuf = NetworkBuffer {
            subject:      Vec::new(),
            slabpresence: Vec::new(),
            memoref:      Vec::new(),
            memo:         Vec::new(),
        };

        netbuf.add_memo(memo);
        netbuf.peerify(from_slab);

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
                    p: self.add_slabpresence(p),
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
            MemoBody::MemoRequest(memorefs, presences) =>{
                MemoBodyBuffer::MemoRequest(
                    memorefs.into_iter().map(|mr| self.add_memoref(mr.memo_id, self.add_subject(mr.subject_id))).collect(),
                    presences.into_iter().map(|p| self.add_slabpresence(p) ).collect()
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
        MemoPeerStateBuffer {
            self.add_slabpresence(),
            peerstate.status
        }
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