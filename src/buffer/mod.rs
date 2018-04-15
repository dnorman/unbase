
//! Buffer Structs, used for intermediate/compact representations
//!
//! Uses offsets inside a given Buffer to more compactly represent data rather than via duplication.
//! This is being processed in memory for now, but it's designed to be a streamed later

pub mod receiver;

//use serde_derive;
use std::collections::HashMap;
use serde_json;
//use serde::ser::Serialize;
//use serde::de::Deserialize;
use futures::{future, stream, prelude::*};

use error::*;
use network::TransportAddress;
use subject::SubjectId;
use slab::{self,prelude::*};
use slab::store::StoreHandle;
use itertools::Itertools;

//use self::receiver::BufferReceiver;

// NOTE: Long run, probably don't want this cloneable.
#[derive(Serialize,Deserialize,Clone)]
pub struct NetworkBuffer {
    segments: Vec<NetbufSegment>,

    // TODO: Make a NetworkBufferBuilder with these fields.
    #[serde(skip)]
    subject_offsets: Vec<(SegmentId,SubjectId)>,
    #[serde(skip)]
    memoref_offsets: Vec<(SegmentId,MemoRef,bool)>,
    #[serde(skip)]
    slabref_offsets: Vec<(SegmentId,SlabRef,bool)>,
}

#[derive(Serialize,Deserialize,Clone)]
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
#[derive(Serialize,Deserialize,Clone)]
pub struct MemoRefBuffer( MemoId, Option<Vec<MemoPeerStateBuffer>>, SubjectOffset );

#[derive(Serialize,Deserialize,Clone)]
pub struct MemoBuffer( MemoRefOffset, Vec<MemoRefOffset>, MemoBodyBuffer );

#[derive(Serialize,Deserialize,Clone)]
pub enum MemoBodyBuffer {
    SlabPresence{ p: SlabPresenceBuffer, r: Vec<MemoRefOffset> }, // TODO: split out root_index_seed conveyance to another memobody type
    Edge(EdgeSetBuffer),
    Edit(HashMap<String, String>),
    FullyMaterialized     { v: HashMap<String, String>, e: EdgeSetBuffer },
    PartiallyMaterialized { v: HashMap<String, String>, e: EdgeSetBuffer },
    Peering(MemoRefOffset,Vec<MemoPeerStateBuffer>),
    MemoRequest(Vec<MemoRefOffset>,Vec<SlabRefOffset>)
}

pub struct SlabRefBuffer {
    pub slab_id: slab::SlabId,
}

#[derive(Serialize,Deserialize,Clone)]
pub struct SlabPresenceBuffer (SlabRefOffset, Vec<TransportAddress>, SlabAnticipatedLifetime);

#[derive(Serialize,Deserialize,Clone)]
pub struct MemoPeerStateBuffer(
    SlabRefOffset,
    MemoPeerStatus
);

#[derive(Serialize,Deserialize,Clone)]
pub struct EdgeSetBuffer (pub HashMap<RelationSlotId, Vec<MemoRefOffset>>);


impl NetworkBuffer{
    pub fn new( from_slabref: SlabRef ) -> Self {
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

        let body: MemoBodyBuffer = match memo.body {
            MemoBody::SlabPresence{ ref s, ref p, ref r } => {
                // TODO: Don't ignore s?
                MemoBodyBuffer::SlabPresence {
                    p: SlabPresenceBuffer(0 /* TODO: offset */, p.addresses, p.lifetime),
                    r: r.iter().map(|memoref| {
                        let subj_offset = self.add_subject(memoref.subject_id);
                        self.add_memoref(memoref, subj_offset)
                    }).collect(), // Assuming parents are the same subject_id as child
                }
            },
            MemoBody::Edge(ref edgeset) => {
                MemoBodyBuffer::Edge(self.add_edge(edgeset))
            },
            MemoBody::Edit(ref hm) => {
                MemoBodyBuffer::Edit(hm.clone())
            },
            MemoBody::FullyMaterialized{ ref v, ref e, .. } => {
                MemoBodyBuffer::FullyMaterialized{ v: v.clone(), e: self.add_edge(e) }
            },
            MemoBody::PartiallyMaterialized{ ref v, ref e, .. } => {
                MemoBodyBuffer::PartiallyMaterialized{ v: v.clone(), e: self.add_edge(e) }
            },
            MemoBody::Peering(ref memoref, ref peerset) => {
                let subj_offset = self.add_subject(memoref.subject_id);
                let memoref_offset = self.add_memoref(memoref, subj_offset);
                MemoBodyBuffer::Peering(
                    memoref_offset,
                    peerset.list.iter().map(|ps| MemoPeerStateBuffer(self.add_slabref(&ps.slabref), ps.status)).collect()
                )
            },
            MemoBody::MemoRequest(ref memorefs, ref slabrefs) =>{
                MemoBodyBuffer::MemoRequest(
                    memorefs.into_iter().map(|memoref|{
                        let subj_offset = self.add_subject(memoref.subject_id);
                        self.add_memoref(memoref, subj_offset)
                    }).collect(),
                    slabrefs.into_iter().map(|r| self.add_slabref(&r) ).collect()
                )
            }
        };

        let subj_offset = self.add_subject(memo.subject_id);

        let buf = MemoBuffer(
            self.add_memoref_and_peerset(memoref, subj_offset, peerset),
            memo.parents.iter().map(|memoref| self.add_memoref(memoref, subj_offset)).collect(), // Assuming parents are the same subject_id as child
            body
        );

        self.segments.push( NetbufSegment::Memo(buf) );
    }
    pub fn populate_memopeersets<F> (&mut self, mut f: F )
        where F: FnMut(&MemoRef) -> MemoPeerSet {

        self.memoref_offsets.drain(..).filter_map(|(seg_id,memoref,added_peerset)|{
            if added_peerset == false {
                Some((seg_id, f(&memoref)))
            }else{
                None
            }
        }).for_each(|(seg_id, peerset)|{

            let peerstates: Vec<MemoPeerStateBuffer> = peerset.list.iter().map(|peerstate| {
                MemoPeerStateBuffer(
                    self.add_slabref(&peerstate.slabref),
                    peerstate.status.clone()
                )
            }).collect();

            if let Some(&mut NetbufSegment::MemoRef(
                MemoRefBuffer(ref _memoId, ref maybe_peerstates, ref _subjectOffset)
            )) = self.segments.get_mut(seg_id as usize) {
                *maybe_peerstates = Some(peerstates);
                //added_peerset = true;
            }
        });
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
        match self.subject_offsets.binary_search_by(|x| x.1.cmp(&subject_id) ){
            Ok(i) => {
                self.subject_offsets[i].0
            }
            Err(i) =>{
                self.segments.push(NetbufSegment::Subject(subject_id));
                let seg_id = (self.segments.len() - 1) as u16;  // TODO truncation

                self.subject_offsets.insert(i,(seg_id, subject_id));

                seg_id
            }
        }
    }
    fn add_memoref_and_peerset (&mut self, memoref: &MemoRef, subj_offset: SubjectOffset, peerset: &MemoPeerSet) -> SegmentId {
        match self.memoref_offsets.binary_search_by(|x| x.1.cmp(memoref) ){
            Ok(i) => {
                self.memoref_offsets[i].0
            }
            Err(i) =>{

                let memopeer_states = peerset.list.iter().map(|peerstate| {
                    MemoPeerStateBuffer(
                        self.add_slabref(&peerstate.slabref ),
                        peerstate.status.clone(),
                    )
                }).collect();

                self.segments.push(NetbufSegment::MemoRef(MemoRefBuffer(memoref.memo_id.clone(), Some(memopeer_states), subj_offset)));
                let seg_id = (self.segments.len() - 1) as u16;  // TODO: Truncation

                self.memoref_offsets.insert(i,(seg_id, memoref.clone(),true));
                seg_id
            }
        }
    }
    fn add_memoref(&mut self, memoref: &MemoRef, subj_offset: SubjectOffset ) -> MemoRefOffset {
        match self.memoref_offsets.binary_search_by(|x| x.1.cmp(memoref) ){
            Ok(i) => {
                self.memoref_offsets[i].0
            }
            Err(i) =>{
                self.segments.push(NetbufSegment::MemoRef(MemoRefBuffer(memoref.memo_id.clone(), None,subj_offset)));
                let seg_id = (self.segments.len() - 1) as u16;  // TODO: Truncation

                self.memoref_offsets.insert(i,(seg_id, memoref.clone(), false));

                seg_id
            }
        }
    }
    fn add_edge(&mut self, e: &EdgeSet) -> EdgeSetBuffer {
        let mut build = HashMap::new();
        for (slot_id,mrh) in e.0 {
            build.insert( slot_id, mrh.iter().map(|memoref| {
                let subj_offset = self.add_subject(memoref.subject_id);
                self.add_memoref(memoref, subj_offset)
            }).collect() );
        }
        EdgeSetBuffer(build)
    }
    fn add_slabref(&mut self, slabref: &SlabRef) -> SegmentId {
        match self.slabref_offsets.binary_search_by(|x| x.1.cmp(slabref) ){
            Ok(i) => {
                self.slabref_offsets[i].0
            }
            Err(i) =>{
                self.segments.push(NetbufSegment::SlabRef(slabref.slab_id));
                let seg_id = (self.segments.len() - 1) as u16;

                self.slabref_offsets.insert(i,(seg_id, slabref.clone(), false));

                seg_id
            }
        }
    }
    // TODO: Maybe this should return ().
    fn add_slabpresence(&mut self, presence: SlabPresence, slabref_offset: SlabRefOffset) -> SegmentId {
        self.segments.push(NetbufSegment::SlabPresence(
            SlabPresenceBuffer(slabref_offset, presence.addresses, presence.lifetime)));
        (self.segments.len() - 1) as u16
    }
    pub fn from_slice(slice: &[u8]) -> Result<Self,Error> {
        serde_json::from_slice(slice).map_err(|e| Error::Serde(e))
    }
    pub fn to_vec(&self) -> Vec<u8> {
        // TODO: Handle error (like above?)
        serde_json::to_vec(self).unwrap()
    }
    pub fn extract(self, receiver: StoreHandle ) -> Box<Future<Item=(),Error=Error>>{
        let segments = self.segments;
        // TODO: Implement.
        for segment in segments {
            match segment {
                NetbufSegment::Subject(subject_id) => {},
                NetbufSegment::SlabRef(_) => {},
                NetbufSegment::MemoRef(_) => {},
                NetbufSegment::Memo(memobuf) => {
                    // memobuf.extract(&segments, receiver)
                },
                NetbufSegment::SlabPresence(_) => {},
            }
        }
//       receiver.put_memo()
        unimplemented!()
    }
}

impl MemoBuffer {
    fn extract (self, segments: &Vec<NetbufSegment>, receiver: StoreHandle ) -> Box<Future<Item=(),Error=Error>> {

    // TODO 1: LEFT OFF HERE

    let my_memo_id : MemoId;
    let my_subject_id : SubjectId;

    let mr_seg_id = self.0;
    let parent_mr_seg_ids = self.1;
    let bodybuf = self.2;

        let my_memo_id;
        let memo_subj_seg_id;
        // TODO: Consider using middle field.
        if let Some(&NetbufSegment::MemoRef(MemoRefBuffer(ref memo_id, _, ref subj_seg_id))) = segments.get(mr_seg_id as usize) {
            my_memo_id = memo_id.clone();
            memo_subj_seg_id = subj_seg_id;

            if let Some(&NetbufSegment::Subject(ref subject_id)) = segments.get(*subj_seg_id as usize) {
                my_subject_id = *subject_id;
            }else {
                return Box::new(future::result(Err(Error::Buffer(BufferError::DecodeFailed))));
            }
        }else {
            return Box::new(future::result(Err(Error::Buffer(BufferError::DecodeFailed))));
        }

        let fut1 = stream::iter_ok::<_, ()>(parent_mr_seg_ids).map(|parent_mr_seg_id|{
            // TODO: Consider renaming middle field.
            if let Some(&NetbufSegment::MemoRef(MemoRefBuffer(ref parent_memo_id, _, ref parent_subj_seg_id))) = segments.get(parent_mr_seg_id as usize) {
                debug_assert_eq!(parent_subj_seg_id, memo_subj_seg_id );
                unimplemented!();  // TODO: Definitely handle this case -- put_memoref needs a peerset.
                //receiver.put_memoref(*parent_memo_id, my_subject_id, MemoPeerSet::empty())
            } else {
                panic!(); // TODO: Handle this case.
            }
        });

        panic!();  // TODO: Handle this case.
        /*
        Box::new(fut1.collect().and_then(|parent_memorefs| {
            let memo = Memo {
                id: my_memo_id,
                subject_id: my_subject_id,
                parents: parent_memorefs,
                body: bodybuf,
            };
            receiver.put_memo(memo, MemoPeerSet::empty())
        }))
        */


    }
}