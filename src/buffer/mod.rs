
//! Buffer Structs, used for intermediate/compact representations
//!
//! Uses offsets inside a given Buffer to more compactly represent data rather than via duplication.
//! This is being processed in memory for now, but it's designed to be a streamed later

pub mod receiver;

use serde_derive;
use std::collections::HashMap;
use serde_json;
use serde::ser::Serialize;
use serde::de::Deserialize;
use std::mem;
use futures::{future, stream, prelude::*};

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
}

type SegmentId = u16;
type MemoRefOffset = SegmentId;
type SlabRefOffset = SegmentId;
type SubjectOffset = SegmentId;

/// `MemoId`, offset of the Memo's subject id, list of slab presence offsets wherein this memo is purportedly present, list of slab presence offsets wherein this memo is peered but not present
#[derive(Serialize,Deserialize)]
struct MemoRefBuffer( MemoId, Option<Vec<MemoPeerStateBuffer>>, SubjectOffset );

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
    SlabRefOffset,
    MemoPeerStatus
);

pub struct EdgeSetBuffer (pub HashMap<RelationSlotId, Vec<MemoRefOffset>>);


impl NetworkBuffer{
    pub fn new() -> Self {
        let len = 1;
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
                    peerset.list.iter().map(|ps| self.make_peerstate(memoref_offset, ps) )
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

                let peerstates = f(memoref).list.iter().map(|peerstate| {
                    MemoPeerStateBuffer(
                        self.add_slabref(&peerstate.slabref),
                        peerstate.status.clone()
                    )
                }).collect();

                if let Some(MemoRefBuffer{ref peerstates}) = self.segments.get(seg_id) {
                    *peerstates = Some(peerstates);
                    mem::replace(added_peerset, true);
                }
            }
        }
    }
    pub fn populate_slabpresences<F> (&mut self, f: F ) where F: FnMut(&MemoRef) -> MemoPeerSet {
        for offset in self.slabref_offsets.drain(..) {
            if offset.2 == false { // did we already add the peerstate?
                //self.add_slabpresence(f.0, f(offset.1));
                unimplemented!()
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
    fn add_memoref_and_peerset (&mut self, memoref: &MemoRef, subj_offset: SubjectOffset, peerset: &MemoPeerSet) -> (SegmentId,Vec<SegmentId>) {
        match self.memoref_offsets.binary_search_by(|x| x.1.cmp(memoref) ){
            Ok(i) => {
                self.memoref_offsets[i].0
            }
            Err(i) =>{

                let memopeer_states = peerset.list.iter().map(|peerstate| {
                    MemoPeerStateBuffer(
                        self.add_slabref(&peerstate.slabref ),
                        peerstate.status
                    )
                }).collect();

                self.segments.push(NetbufSegment::MemoRef(MemoRefBuffer(memoref.memo_id.clone(), Some(memopeer_states), subj_offset)));
                let seg_id = self.segments.len() - 1;

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
                self.segments.push(NetbufSegment::MemoRef(MemoRefBuffer(memoref.memo_id.clone(), None,subj_offset)));
                let seg_id = self.segments.len() - 1;

                self.memoref_offsets.insert(i,(seg_id, memoref.clone(), false));

                seg_id
            }
        }
    }
    fn add_edge(&mut self, e: EdgeSet) -> EdgeSetBuffer {
        let mut e = HashMap::new();
        for (slot_id,mrh) in e {
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
    fn add_slabpresence(&mut self, presence: SlabPresence, slabref_offset: SlabRefOffset) -> SegmentId {
        self.segments.push(NetbufSegment::SlabPresence(SlabPresenceBuffer(slabref_offset, presence.addresses, presence.lifetime)));
    }
    pub fn from_slice(slice: &[u8]) -> Result<Self,Error> {
        serde_json::from_slice(slice).map_err(|e| Error::Serde(e))
    }
    pub fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self)
    }
    pub fn extract(self, receiver: &mut impl StorageCoreInterface ) -> Box<Future<Item=(),Error=Error>>{
        let segments = self.segments;
        for segment in segments.iter() {
            match segment {
                NetbufSegment::Subject(subject_id) => {},
                NetbufSegment::SlabRef(_) => {},
                NetbufSegment::MemoRef(MemoRefBuffer{}) => {},
                NetbufSegment::Memo(memobuf) => {
                    memobuf.extract(&segments, receiver)
                },
                NetbufSegment::SlabPresence(_) => {},
                NetbufSegment::MemoPeerState(_) => {},
            }
        }
//       receiver.put_memo()
        unimplemented!()
    }
}

impl MemoBuffer {
    fn extract (self, &segments: Vec<NetbufSegment>, receiver: &mut impl StorageCoreInterface ) -> Box<Future<Item=(),Error=Error>> {


        // TODO 1: LEFT OFF HERE

        let my_memo_id;
        let my_subject_id;

        if let MemoBuffer( mr_seg_id, parent_mr_seg_ids, bodybuf ) = self {
            let my_memo_id;
            let memo_subj_seg_id;
            if let Some(MemoRefBuffer(ref memo_id, ref subj_seg_id)) = segments.get(mr_seg_id) {
                my_memo_id = memo_id.clone();
                memo_subj_seg_id = subj_seg_id;

                if let Some(subject_id @ SubjectId) = segments.get(subj_seg_id) {
                    my_subject_id = subject_id;
                }else {
                    return future::result(Err(Error::Buffer(BufferError::DecodeFailed)))
                }
            }else {
                return future::result(Err(Error::Buffer(BufferError::DecodeFailed)))
            }

            stream::iter_ok::<_, ()>(parent_mr_seg_ids).map(|parent_mr_seg_id|{
                if let Some(MemoRefBuffer(ref parent_memo_id, ref parent_subj_seg_id)) = segments.get(parent_mr_seg_id) {
                    debug_assert_eq!(parent_subj_seg_id, memo_subj_seg_id );
                    receiver.put_memoref(parent_memo_id, my_subject_id)
                }
            }).collect().and_then(|parent_memorefs| {
                let memo = Memo {
                    id: my_memo_id,
                    subject_id: my_subject_id,
                    parents: parent_memorefs,
                    body: bodybuf,
                };
                receiver.put_memo(memo, MemoPeerSet::empty(), )
            })
        }else {
            return future::result(Err(Error::Buffer(BufferError::DecodeFailed)))
        }

    }
}