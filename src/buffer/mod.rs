
//! Buffer Structs, used for intermediate/compact representations
//!
//! Uses offsets inside a given Buffer to more compactly represent data rather than via duplication.
//! This is being processed in memory for now, but it's designed to be a streamed later

pub mod receiver;

use serde_derive;
use std::collections::HashMap;
use serde_json;
pub use serde::ser::Serialize;
pub use serde::de::Deserialize;

use error::*;
use network::TransportAddress;
use subject::SubjectId;
use slab::{self,prelude::*};
use slab::storage::StorageCoreInterface;

use self::receiver::BufferReceiver;

#[derive(Serialize,Deserialize)]
pub struct NetworkBuffer {
    segments: Vec<NetbufSegment>,

    #[serde(skip)]
    subject_offsets: Vec<(SegmentId,SubjectId)>,
    #[serde(skip)]
    memoref_offsets: Vec<(SegmentId,MemoRef,bool)>,
    #[serde(skip)]
    slabref_offsets: Vec<(SegmentId,SlabRef,bool)>,
}

pub enum NetbufSegment {
    Subject(SubjectId),
    SlabRef(slab::SlabId),
    MemoRef(MemoRefBuffer),
    Memo(MemoBuffer),
    SlabPresence(SlabPresenceBuffer),
    MemoPeerState(MemoPeerStateBuffer),
}

type SegmentId = u16;
type MemoRefOffset = SegmentId;
type SlabRefOffset = SegmentId;
type SubjectOffset = SegmentId;

/// `MemoId`, offset of the Memo's subject id, list of slab presence offsets wherein this memo is purportedly present, list of slab presence offsets wherein this memo is peered but not present
#[derive(Serialize,Deserialize)]
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
    pub slab_id: slab::SlabId,
}

#[derive(Serialize,Deserialize)]
struct SlabPresenceBuffer (SlabRefOffset, Vec<TransportAddress>, SlabAnticipatedLifetime);

#[derive(Serialize,Deserialize)]
struct MemoPeerStateBuffer(
    MemoRefOffset,
    SlabRefOffset,
    MemoPeerStatus
);

pub struct EdgeSetBuffer (pub HashMap<RelationSlotId, Vec<MemoRefOffset>>);


impl NetworkBuffer{
    pub fn new() -> Self {
        let len = memos.len();
        NetworkBuffer{
            segments: Vec::with_capacity( 8 + (len * 6) ),  // Memo + Self memoref+peerset + Parent Memoref+peerset + Subject + Slabref + SlabPresence
            memoref_offsets: Vec::with_capacity( len * 2), // Most memorefs have one parent
            subject_offsets: Vec::with_capacity( len ),     // Might actually be generous
            slabref_offsets: Vec::with_capacity( 8 ),               // Probably not more than eight total
        }
    }
    pub fn add_memoref_peerset_and_memo(&mut self, memoref: &MemoRef, peerset: &MemoPeerSet, memo: &Memo, ) {
        // QUESTION: Should we dedup memos here?

        use slab::memo::MemoBody::*;
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
            MemoBody::FullyMaterialized{ v, e, .. } => {
                MemoBodyBuffer::FullyMaterialized{ v, e: self.add_edge(e) }
            },
            MemoBody::PartiallyMaterialized{ v, e, .. } => {
                MemoBodyBuffer::PartiallyMaterialized{ v, e: self.add_edge(e) }
            },
            MemoBody::Peering(mr, peerset) => {
                let memoref_offset = self.add_memoref(mr.memo_id, self.add_subject(mr.subject_id));
                MemoBodyBuffer::Peering(
                    memoref_offset,
                    peerset.list.iter().map(|ps| self.add_peerstate(memoref_offset, ps) )
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

        let buf = MemoBuffer(
            self.add_memoref_and_peerset(memoref, subj_offset, peerset),
            memo.parents.iter().map(|mr| self.add_memoref( mr.memo_id, subj_offset) ).collect(), // Assuming parents are the same subject_id as child
            body
        );

        self.segments.push( NetbufSegment::Memo(buf) );
    }
    pub fn populate_memopeersets<F> (&mut self, f: F ) where F: FnMut(&MemoRef) -> MemoPeerSet {
        for (seg_id,memoref,added_peerset) in self.memoref_offsets.drain(..) {

            if added_peerset == false {
                for peerstate in f(memoref).list.iter(){
                    self.add_peerstate(seg_id, peerstate);
                }
            }
        }
    }
    pub fn populate_slabpresences<F> (&mut self, f: F ) where F: FnMut(&MemoRef) -> MemoPeerSet {
        for offset in self.memoref_offsets.drain(..) {
            if offset.2 == false { // did we already add the peerstate?
                self.add_peerstate(f.0, f(offset.1));
            }
        }
    }
    pub fn is_fully_populated (&self) -> bool {
        self.memoref_offsets.iter().find_position(|o| o.2 == false ).is_none() &&
            self.slabref_offsets.iter().find_position(|o| o.2 == false ).is_none()
    }
    fn add_subject(&mut self, subject_id: SubjectId) -> SubjectOffset {
        match self.subject.iter().rposition(&subject_id) {
            Some(i) => i,
            None =>{
                self.segments.push(subject_id);
                self.segments.len() - 1
            }
        }
    }
    fn add_memoref_and_peerset (&mut self, memoref: &MemoRef, subj_offset: SubjectOffset, peerset: &MemoPeerSet) -> SegmentId {
        match self.memoref_offsets.binary_search_by(|x| x.1.cmp(memoref) ){
            Ok(i) => {
                self.memoref_offsets[i].0
            }
            Err(i) =>{
                self.segments.push(NetbufSegment::MemoRef(MemoRefBuffer(memoref.memo_id.clone(), subj_offset)));
                let seg_id = self.segments.len() - 1;

                for peerstate in peerset.list.iter(){
                    self.add_peerstate(seg_id, peerstate);
                }

                self.memoref_offsets.insert(i,(seg_id, memoref.clone(),true));
                seg_id
            }
        }
    }
    fn add_memoref (&mut self, memoref: MemoRef, subj_offset: SubjectOffset ) -> MemoRefOffset {
        match self.memoref_offsets.binary_search_by(|x| x.1.cmp(memoref) ){
            Ok(i) => {
                self.memoref_offsets[i].0
            }
            Err(i) =>{
                self.segments.push(NetbufSegment::MemoRef(MemoRefBuffer(memoref.memo_id.clone(), subj_offset)));
                let seg_id = self.segments.len() - 1;

                self.memoref_offsets.insert(i,(seg_id, memoref.clone(), false));

                seg_id
            }
        }
    }
    fn add_peerstate(&mut self, memoref_offset: MemoRefOffset, peerstate: &MemoPeerState ) -> MemoPeerStateBuffer {
        MemoPeerStateBuffer(
            memoref_offset,
            self.add_slabref( &peerstate.slabref ),
            peerstate.status.clone()
        )
    }
    fn add_edge(&mut self, e: EdgeSet) -> EdgeSetBuffer {
        let mut e = HashMap::new();
        for (slot_id,mrh) in edgeset {
            e.insert( slot_id, mrh.map(|mr| self.add_memoref(mr.memo_id, self.add_subject(mr.subject_id))).collect() );
        }
        MemoBodyBuffer::Edge(e)
    }
    fn add_slabref(&mut self, slabref: &SlabRef) -> SegmentId {

        match self.slabref_accum.binary_search_by(|x| x.1.cmp(&slabref) ){
            Ok(i) => {
                self.slabref_offset_accum[i]
            }
            Err(i) =>{
                self.segments.push(NetbufSegment::SlabRef(slabref.slab_id));
                let seg_id = self.slabpresence.len() - 1;

                self.slabref_offsets.insert(i,(seg_id, slabref.clone(),false));

                seg_id
            }
        }
    }
    fn add_slabpresence(&mut self, presence: SlabPresence, slabref_offset: SlabRefOffset) -> SlabPresenceOffset {
        self.segments.push(NetbufSegment::SlabPresence(SlabPresenceBuffer(slabref_offset, presence.addresses, presence.lifetime)));
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