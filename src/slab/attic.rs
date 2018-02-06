#[derive(Clone,Debug)]
pub struct MemoPeerList(pub Vec<MemoPeer>);

impl MemoPeerList {
    pub fn new(list: Vec<MemoPeer>) -> Self {
        MemoPeerList(list)
    }
    pub fn clone(&self) -> Self {
        MemoPeerList(self.0.clone())
    }
    pub fn clone_for_slab(&self, to_slab: &LocalSlabHandle) -> Self {
        MemoPeerList(self.0
            .iter()
            .map(|p| {
                MemoPeer {
                    slabref: p.slab_id, //.clone_for_slab(to_slab),
                    status: p.status.clone(),
                }
            })
            .collect())
    }
    pub fn slabrefs(&self) -> Vec<SlabRef> {
        self.0.iter().map(|p| p.slabref).collect()
    }
    // pub fn apply_peer(&mut self, peer: MemoPeer) -> bool {
    //     // assert!(self.owning_slab_id == peer.slabref.owning_slab_id, "apply_peer for dissimilar owning_slab_id peer" );

    //     let peerlist = &mut self.0;
    //     {
    //         if let Some(my_peer) = peerlist.iter_mut()
    //             .find(|p| p.slab_id == peer.slab_id) {
    //             if peer.status != my_peer.status {
    //                 // same slabref, so no need to apply the peer presence
    //                 my_peer.status = peer.status;
    //                 return true;
    //             } else {
    //                 return false;
    //             }
    //         }
    //     }

    //     peerlist.push(peer);
    //     true
    // }
}

impl Deref for MemoPeerList {
    type Target = Vec<MemoPeerState>;
    fn deref(&self) -> &Vec<MemoPeerState> {
        &self.0
    }
}



*** FROM MemoRef: ***
    pub fn exceeds_min_durability_threshold (&self) -> bool {
        let peerlist = self.peerlist.read().unwrap();
        // TODO - implement proper durability estimation logic
        return peerlist.len() > 0
    }
    // pub fn apply_peers ( &self, apply_peerlist: &MemoPeerList ) -> bool {

    //     let mut peerlist = self.peerlist.write().unwrap();
    //     let mut acted = false;
    //     for apply_peer in apply_peerlist.0.clone() {
    //         if apply_peer.slabref.slab_id == self.owning_slab_id {
    //             println!("WARNING - not allowed to apply self-peer");
    //             //panic!("memoref.apply_peers is not allowed to apply for self-peers");
    //             continue;
    //         }
    //         if peerlist.apply_peer(apply_peer) {
    //             acted = true;
    //         }
    //     }
    //     acted
    // }
    pub fn is_peered_with_slabref(&self, slabref: &SlabRef) -> bool {
        let status = self.peerlist.read().unwrap().iter().any(|peer| {
            (peer.slabref.0.slab_id == slabref.0.slab_id && peer.status != MemoPeerStatus::NonParticipating)
        });

        status
    }
    pub fn want_peer_count (&self) -> u32 {
        // TODO: test each memo for durability_score and emit accordingly

        match self.subject_id {
            None    => 0,
            // TODO - make this number dynamic on the basis of estimated durability
            Some(_) => (2 as u32).saturating_sub( self.peerlist.read().unwrap().len() as u32 )
        }
    }
    pub fn update_peer (&self, slabref: &SlabRef, status: MemoPeerStatus) -> bool {

        let mut acted = false;
        let mut found = false;
        let ref mut list = self.peerlist.write().unwrap().0;
        for peer in list.iter_mut() {
            if peer.slabref.slab_id == self.owning_slab_id {
                println!("WARNING - not allowed to apply self-peer");
                //panic!("memoref.update_peers is not allowed to apply for self-peers");
                continue;
            }
            if peer.slabref.slab_id == slabref.slab_id {
                found = true;
                if peer.status != status {
                    acted = true;
                    peer.status = status.clone();
                }
                // TODO remove the peer entirely for MemoPeerStatus::NonParticipating
                // TODO prune excess peers - Should keep this list O(10) peers
            }
        }

        if !found {
            acted = true;
            list.push(MemoPeerState{
                slabref: slabref.clone(),
                status: status.clone()
            })
        }

        acted
    }