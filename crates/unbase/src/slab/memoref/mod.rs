use core::ops::Deref;

pub mod serde;
use super::*;
use crate::{
    error::RetrieveError,
    head::Head,
};

use std::{
    fmt,
    sync::{
        Arc,
        RwLock,
    },
};
use tracing::warn;

#[derive(Clone)]
pub struct MemoRef(pub Arc<MemoRefInner>);

impl Deref for MemoRef {
    type Target = MemoRefInner;

    fn deref(&self) -> &MemoRefInner {
        &*self.0
    }
}

pub struct MemoRefInner {
    pub id:             MemoId,
    pub owning_slab_id: SlabId, // TODO - rename and conditionalize with a macro
    pub entity_id:      Option<EntityId>,
    pub peerlist:       RwLock<MemoPeerList>,
    pub ptr:            RwLock<MemoRefPtr>,
}

#[derive(Debug)]
pub enum MemoRefPtr {
    Resident(Memo),
    Remote,
}

impl MemoRefPtr {
    pub fn to_peering_status(&self) -> MemoPeeringStatus {
        match self {
            &MemoRefPtr::Resident(_) => MemoPeeringStatus::Resident,
            &MemoRefPtr::Remote => MemoPeeringStatus::Participating,
        }
    }
}

impl MemoRef {
    pub fn to_head(&self) -> Head {
        match self.entity_id {
            None => {
                Head::Anonymous { owning_slab_id: self.owning_slab_id,
                                  head:           vec![self.clone()], }
            },
            Some(entity_id) => {
                Head::Entity { owning_slab_id: self.owning_slab_id,
                               entity_id,
                               head: vec![self.clone()] }
            },
        }
    }

    pub fn apply_peers(&self, apply_peerlist: &MemoPeerList) -> bool {
        let peerlist = &mut *self.peerlist.write().unwrap();
        let mut acted = false;
        for apply_peer in apply_peerlist.0.clone() {
            if apply_peer.slabref.slab_id == self.owning_slab_id {
                warn!("WARNING - not allowed to apply self-peer");
                // panic!("memoref.apply_peers is not allowed to apply for self-peers");
                continue;
            }
            if peerlist.apply_peer(apply_peer) {
                acted = true;
            }
        }
        acted
    }

    pub fn get_peerlist_for_peer(&self, my_ref: &SlabRef, maybe_dest_slab_id: Option<SlabId>) -> MemoPeerList {
        let mut list: Vec<MemoPeer> = Vec::new();

        list.push(MemoPeer { slabref: my_ref.clone(),
                             status:  self.ptr.read().unwrap().to_peering_status(), });

        // Tell the peer about all other presences except for ones belonging to them
        // we don't need to tell them they have it. They know, they were there :)

        if let Some(dest_slab_id) = maybe_dest_slab_id {
            for peer in self.peerlist.read().unwrap().iter() {
                if peer.slabref.0.slab_id != dest_slab_id {
                    list.push((*peer).clone());
                }
            }
        } else {
            list.append(&mut self.peerlist.read().unwrap().0.clone());
        }

        MemoPeerList::new(list)
    }

    pub fn is_resident(&self) -> bool {
        match *self.ptr.read().unwrap() {
            MemoRefPtr::Resident(_) => true,
            _ => false,
        }
    }

    pub fn get_memo_if_resident(&self) -> Option<Memo> {
        match *self.ptr.read().unwrap() {
            MemoRefPtr::Resident(ref memo) => Some(memo.clone()),
            _ => None,
        }
    }

    pub fn is_peered_with_slabref(&self, slabref: &SlabRef) -> bool {
        let status =
            self.peerlist
                .read()
                .unwrap()
                .iter()
                .any(|peer| peer.slabref.0.slab_id == slabref.0.slab_id && peer.status != MemoPeeringStatus::NonParticipating);

        status
    }

    #[tracing::instrument(level = "debug")]
    pub async fn get_memo(self, slab: SlabHandle) -> Result<Memo, RetrieveError> {
        if self.owning_slab_id != slab.my_ref.slab_id {
            assert!(self.owning_slab_id == slab.my_ref.slab_id,
                    "requesting slab does not match owning slab");
        }

        // This seems pretty crude, but using channels for now in the interest of expediency
        {
            if let MemoRefPtr::Resident(ref memo) = *self.ptr.read().unwrap() {
                return Ok(memo.clone());
            }
        }

        slab.request_memo(self.clone()).await
    }

    #[tracing::instrument]
    pub async fn descends(&self, memoref: &MemoRef, slab: &SlabHandle) -> Result<bool, RetrieveError> {
        assert!(self.owning_slab_id == slab.my_ref.slab_id);
        // TODO get rid of clones here

        if self.clone().get_memo(slab.clone()).await?.descends(&memoref, slab).await? {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // TODO - move this to slab
    pub fn update_peer(&self, slabref: &SlabRef, status: MemoPeeringStatus) -> bool {
        let mut acted = false;
        let mut found = false;
        let ref mut list = self.peerlist.write().unwrap().0;
        for peer in list.iter_mut() {
            if peer.slabref.slab_id == self.owning_slab_id {
                warn!("WARNING - not allowed to apply self-peer");
                // panic!("memoref.update_peers is not allowed to apply for self-peers");
                continue;
            }
            if peer.slabref.slab_id == slabref.slab_id {
                found = true;
                if peer.status != status {
                    acted = true;
                    peer.status = status.clone();
                }
                // TODO remove the peer entirely for MemoPeeringStatus::NonParticipating
                // TODO prune excess peers - Should keep this list O(10) peers
            }
        }

        if !found {
            acted = true;
            list.push(MemoPeer { slabref: slabref.clone(),
                                 status:  status.clone(), })
        }

        acted
    }
    //    pub fn want_peer_count (&self) -> u32 {
    //        // TODO: test each memo for durability_score and emit accordingly
    //
    //        match self.subject_id() {
    //            None => 0,
    //            // TODO - make this number dynamic on the basis of estimated durability
    //            Some(_) => (2 as u32).saturating_sub(self.peerlist.read().unwrap().len() as u32)
    //        }
    //    }
}

impl fmt::Debug for MemoRef {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("MemoRef")
           .field("id", &self.id)
           .field("owning_slab_id", &self.owning_slab_id)
           .field("entity_id", &self.entity_id)
           .field("peerlist", &*self.peerlist.read().unwrap())
           .field("memo", &*self.ptr.read().unwrap())
           .finish()
    }
}

impl PartialEq for MemoRef {
    fn eq(&self, other: &MemoRef) -> bool {
        // TODO: handle the comparision of pre-hashed memos as well as hashed memos
        self.id == other.id
    }
}

impl Drop for MemoRefInner {
    fn drop(&mut self) {
        //
    }
}
