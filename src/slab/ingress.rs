use super::*;

impl Slab {
    // NOTE: this is run inside a dedicated thread, as fetches from other slabs may be required for
    // apply_head ( which calls descends, which calls get_memo, which blocks )
    // QUESTION: could this be managed with a marker?
    pub fn dispatch_memoref (&self, memoref : MemoRef){
        //println!("# \t\\ Slab({}).dispatch_memoref({})", self.id, &memoref.id );

        if let Some(subject_id) = memoref.subject_id {
            // TODO2 - switch network modules over to use tokio, ingress to use tokio mpsc stream
            // TODO: Switch subject subscription mechanism to be be based on channels, and matching trees
            // subject_subscriptions: Mutex<HashMap<SubjectId, Vec<mpsc::Sender<Option<MemoRef>>>>>

            if let Some(ref senders) = self.subject_subscriptions.lock().unwrap().get( &subject_id ) {
                for sender in senders.iter(){
                    //TODO2 - what to do here when sending fails?
                    sender.send(Some(memoref.clone())).unwrap();
                }
            }
        }
    }

    //NOTE: nothing that calls get_memo, directly or indirectly is presently allowed here (but get_memo_if_resident is ok)
    pub fn handle_memo_from_other_slab( &self, memo: &Memo, memoref: &MemoRef, origin_slabref: &SlabRef ){
        println!("Slab({}).handle_memo_from_other_slab({})", self.id, memo.id );

        // TODO - how do we handle FullyMaterialized, Edge and other memobodies for items which are:
        // 1. Index Nodes?
        // 2. In one or more of our contexts?
        // And why isn't the hack_send_context method working for subject 9003/memo 5003? ( index branch )

        match memo.body {
            // This Memo is a peering status update for another memo
            MemoBody::SlabPresence{ p: ref presence, r: ref root_index_seed } => {

                //TODO1 - finish converting all Option<MRH> to MRH::Null / Other
                match root_index_seed {
                    &MemoRefHead::Subject{..} | &MemoRefHead::Anonymous{..} => {
                        // HACK - this should be done inside the deserialize
                        for memoref in root_index_seed.iter() {
                            memoref.update_peer(origin_slabref, MemoPeeringStatus::Resident);
                        }

                        self.net.apply_root_index_seed( &presence, root_index_seed, &self.my_ref );
                    }
                    &MemoRefHead::Null => {}
                }

                let mut reply = false;
                if let &MemoRefHead::Null = root_index_seed {
                    reply = true;
                }

                if reply {
                    if let Ok(mentioned_slabref) = self.slabref_from_presence( presence ) {
                        // TODO: should we be telling the origin slabref, or the presence slabref that we're here?
                        //       these will usually be the same, but not always

                        let my_presence_memoref = self.new_memo_basic(
                            None,
                            memoref.to_head(),
                            MemoBody::SlabPresence{
                                p: self.presence_for_origin( origin_slabref ),
                                r: self.get_root_index_seed()
                            }
                        );

                        origin_slabref.send( &self.my_ref, &my_presence_memoref );

                        let _ = mentioned_slabref;
                        // needs PartialEq
                        //if mentioned_slabref != origin_slabref {
                        //   mentioned_slabref.send( &self.my_ref, &my_presence_memoref );
                        //}
                    }
                }
            }
            MemoBody::Peering(memo_id, subject_id, ref peerlist ) => {
                let (peered_memoref,_had_memo) = self.assert_memoref( memo_id, subject_id, peerlist.clone(), None );

                // Don't peer with yourself
                for peer in peerlist.iter().filter(|p| p.slabref.0.slab_id != self.id ) {
                    peered_memoref.update_peer( &peer.slabref, peer.status.clone());
                }
            },
            MemoBody::MemoRequest(ref desired_memo_ids, ref requesting_slabref ) => {

                if requesting_slabref.0.slab_id != self.id {
                    for desired_memo_id in desired_memo_ids {
                        if let Some(desired_memoref) = self.memorefs_by_id.read().unwrap().get(&desired_memo_id) {

                            if desired_memoref.is_resident() {
                                requesting_slabref.send(&self.my_ref, desired_memoref)
                            } else {
                                // Somebody asked me for a memo I don't have
                                // It would be neighborly to tell them I don't have it
                                self.do_peering(&memoref,requesting_slabref);
                            }
                        }else{
                            let peering_memoref = self.new_memo(
                                None,
                                memoref.to_head(),
                                MemoBody::Peering(
                                    *desired_memo_id,
                                    None,
                                    MemoPeerList::new(vec![MemoPeer{
                                        slabref: self.my_ref.clone(),
                                        status: MemoPeeringStatus::NonParticipating
                                    }])
                                )
                            );
                            requesting_slabref.send(&self.my_ref, &peering_memoref)
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
