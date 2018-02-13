use slab::{prelude::*};

#[derive(Clone, Debug)]
pub struct MemoPeerState {
    pub slabref: SlabRef,
    pub status: MemoPeerStatus,
}

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub enum MemoPeerStatus {
    Resident,
    Participating,
    NonParticipating,
    Unknown,
}

#[derive(Clone,Debug)]
pub struct MemoPeerSet{
    list: Vec<MemoPeerState>,
}

impl MemoPeerSet {
    pub fn new(list: Vec<MemoPeerState>) -> Self {
        MemoPeerSet{
            list,
        }
    }
    pub fn clone(&self) -> Self {
        MemoPeerSet{
            list: self.list.clone(),
        }
    }
    pub fn slabrefs(&self) -> Vec<SlabRef> {
        self.list.iter().map(|p| p.slabref).collect()
    }
    pub fn apply_peerstate(&mut self, peerstate: MemoPeerState) -> bool {
        // assert!(self.owning_slab_id == peer.slabref.owning_slab_id, "apply_peer for dissimilar owning_slab_id peer" );

        let peerlist = &mut self.list;
        {
            if let Some(my_peerstate) = peerlist.iter_mut().find(|p| p.slabref == peerstate.slabref) {
                if peerstate.status != my_peerstate.status {
                    // same slabref, so no need to apply the peer presence
                    my_peerstate.status = peerstate.status;
                    return true;
                } else {
                    return false;
                }
            }
        }

        peerlist.push(peerstate);
        true
    }
    pub fn apply_peerset ( &self, apply_peerset: &MemoPeerSet ) -> bool {

        let mut acted = false;
        for peerstate in apply_peerset.list {
            if self.apply_peerstate(peerstate) {
                acted = true;
            }
        }
        
        acted
    }
    pub fn for_slabref(&self, slabref: &SlabRef) -> Self {
        MemoPeerSet{
            list: self.list.iter().filter(|p| p.slabref != *slabref).map(|p| p.clone()).collect()
        }
    }
    pub fn clone_for_slab(&self, slab: &LocalSlabHandle) -> Self {
        MemoPeerSet{
            list: self.list.iter().map(|p| p.clone_for_slab(slab) ).collect()
        }
    }
}

impl MemoPeerState {
    pub fn clone_for_slab(&self, to_slab: &LocalSlabHandle) -> Self {
        MemoPeerState{
            status: self.status,
            slabref: self.slabref.clone_for_slab(to_slab)
        }
    }
}

impl <'a> Into<&'a Vec<MemoPeerState>> for &'a MemoPeerSet {
    fn into(self) -> &'a Vec<MemoPeerState> {
        &self.list
    }
}