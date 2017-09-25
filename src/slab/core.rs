use super::*;


impl Slab {
    pub fn new_memo ( &self, subject_id: Option<SubjectId>, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        let mut counters = self.counters.write().unwrap();
        counters.last_memo_id += 1;
        let memo_id = (self.id as u64).rotate_left(32) | counters.last_memo_id as u64;

        //println!("# Slab({}).new_memo(id: {},subject_id: {:?}, parents: {:?}, body: {:?})", self.id, memo_id, subject_id, parents.memo_ids(), body );

        let memo = Memo::new(MemoInner {
            id:    memo_id,
            owning_slab_id: self.id,
            subject_id: subject_id,
            parents: parents,
            body: body
        });

        let (memoref, _had_memoref) = self.assert_memoref(memo.id, memo.subject_id, MemoPeerList(Vec::new()), Some(memo) );
        self.consider_emit_memo(&memoref);

        memoref
    }
    pub fn reconstitute_memo ( &self, memo_id: MemoId, subject_id: Option<SubjectId>, parents: MemoRefHead, body: MemoBody, origin_slabref: &SlabRef, peerlist: &MemoPeerList ) -> (Memo,MemoRef,bool){
        //println!("Slab({}).reconstitute_memo({})", self.id, memo_id );
        // TODO: find a way to merge this with assert_memoref to avoid doing duplicative work with regard to peerlist application

        let memo = Memo::new(MemoInner {
            id:             memo_id,
            owning_slab_id: self.id,
            subject_id:     subject_id,
            parents:        parents,
            body:           body
        });

        let (memoref, had_memoref) = self.assert_memoref(memo.id, memo.subject_id, peerlist.clone(), Some(memo.clone()) );

        {
            let mut counters = self.counters.write().unwrap();
            counters.memos_received += 1;
            if had_memoref {
                counters.memos_redundantly_received += 1;
            }
        }
        //println!("Slab({}).reconstitute_memo({}) B -> {:?}", self.id, memo_id, memoref );


        self.consider_emit_memo(&memoref);

        if let Some(ref memo) = memoref.get_memo_if_resident() {

            self.check_memo_waiters(memo);
            self.handle_memo_from_other_slab(memo, &memoref, &origin_slabref);
            self.do_peering(&memoref, &origin_slabref);

        }

        if let Some(ref tx_mutex) = self.memoref_dispatch_tx_channel {
            tx_mutex.lock().unwrap().send(memoref.clone()).unwrap()
        }

        (memo, memoref, had_memoref)
    }
    pub fn residentize_memoref(&self, memoref: &MemoRef, memo: Memo) -> bool {
        //println!("# Slab({}).MemoRef({}).residentize()", self.id, memoref.id);

        assert!(memoref.owning_slab_id == self.id);
        assert!( memoref.id == memo.id );

        let mut ptr = memoref.ptr.write().unwrap();

        if let MemoRefPtr::Remote = *ptr {
            *ptr = MemoRefPtr::Resident( memo );

            // should this be using do_peering_for_memo?
            // doing it manually for now, because I think we might only want to do
            // a concise update to reflect our peering status change

            let peering_memoref = self.new_memo(
                None,
                memoref.to_head(),
                MemoBody::Peering(
                    memoref.id,
                    memoref.subject_id,
                    MemoPeerList::new(vec![ MemoPeer{
                        slabref: self.my_ref.clone(),
                        status: MemoPeeringStatus::Resident
                    }])
                )
            );

            for peer in memoref.peerlist.read().unwrap().iter() {
                peer.slabref.send( &self.my_ref, &peering_memoref );
            }

            // residentized
            true
        }else{
            // already resident
            false
        }
    }
    pub fn remotize_memoref( &self, memoref: &MemoRef ) -> Result<(),String> {
        assert!(memoref.owning_slab_id == self.id);

        //println!("# Slab({}).MemoRef({}).remotize()", self.id, memoref.id );

        // TODO: check peering minimums here, and punt if we're below threshold

        let send_peers;
        {
            let mut ptr = memoref.ptr.write().unwrap();
            if let MemoRefPtr::Resident(_) = *ptr {
                let peerlist = memoref.peerlist.read().unwrap();

                if peerlist.len() == 0 {
                    return Err("Cannot remotize a zero-peer memo".to_string());
                }
                send_peers = peerlist.clone();
                *ptr = MemoRefPtr::Remote;

            }else{
                return Ok(());
            }
        }

        let peering_memoref = self.new_memo_basic(
            None,
            memoref.to_head(),
            MemoBody::Peering(
                memoref.id,
                memoref.subject_id,
                MemoPeerList::new(vec![MemoPeer{
                    slabref: self.my_ref.clone(),
                    status: MemoPeeringStatus::Participating
                }])
            )
        );

        //self.consider_emit_memo(&memoref);

        for peer in send_peers.iter() {
            peer.slabref.send( &self.my_ref, &peering_memoref );
        }

        Ok(())
    }
    pub fn request_memo (&self, memoref: &MemoRef) -> u8 {
        //println!("Slab({}).request_memo({})", self.id, memoref.id );

        let request_memo = self.new_memo_basic(
            None,
            MemoRefHead::Null,
            MemoBody::MemoRequest(
                vec![memoref.id],
                self.my_ref.clone()
            )
        );

        let mut sent = 0u8;
        for peer in memoref.peerlist.read().unwrap().iter().take(5) {
            //println!("Slab({}).request_memo({}) from {}", self.id, memoref.id, peer.slabref.slab_id );
            peer.slabref.send( &self.my_ref, &request_memo.clone() );
            sent += 1;
        }

        sent
    }
    pub fn assert_memoref( &self, memo_id: MemoId, subject_id: Option<SubjectId>, peerlist: MemoPeerList, memo: Option<Memo>) -> (MemoRef, bool) {

        let had_memoref;
        let memoref = match self.memorefs_by_id.write().unwrap().entry(memo_id) {
            Entry::Vacant(o)   => {
                let mr = MemoRef(Arc::new(
                    MemoRefInner {
                        id: memo_id,
                        owning_slab_id: self.id,
                        subject_id: subject_id,
                        peerlist: RwLock::new(peerlist),
                        ptr:      RwLock::new(match memo {
                            Some(m) => {
                                assert!(self.id == m.owning_slab_id);
                                MemoRefPtr::Resident(m)
                            }
                            None    => MemoRefPtr::Remote
                        })
                    }
                ));

                had_memoref = false;
                o.insert( mr ).clone()// TODO: figure out how to prolong the borrow here & avoid clone
            }
            Entry::Occupied(o) => {
                let mr = o.get();
                had_memoref = true;
                if let Some(m) = memo {

                    let mut ptr = mr.ptr.write().unwrap();
                    if let MemoRefPtr::Remote = *ptr {
                        *ptr = MemoRefPtr::Resident(m)
                    }
                }
                mr.apply_peers( &peerlist );
                mr.clone()
            }
        };

        (memoref, had_memoref)
    }
    pub fn assert_slabref(&self, slab_id: SlabId, presence: &[SlabPresence] ) -> SlabRef {
        //println!("# Slab({}).assert_slabref({}, {:?})", self.id, slab_id, presence );

        if slab_id == self.id {
            return self.my_ref.clone();
            // don't even look it up if it's me.
            // We must not allow any third party to edit the peering.
            // Also, my ref won't appeara in the list of peer_refs, because it's not a peer
        }

        let maybe_slabref = {
            // Instead of having to scope our read lock, and getting a write lock later
            // should we be using a single write lock for the full function scope?
            if let Some(slabref) = self.peer_refs.read().expect("peer_refs.read()").iter().find(|r| r.0.slab_id == slab_id ){
                Some(slabref.clone())
            }else{
                None
            }
        };

        let slabref : SlabRef;
        if let Some(s) = maybe_slabref {
            slabref = s;
        }else{
            let inner = SlabRefInner {
                slab_id:        slab_id,
                owning_slab_id: self.id, // for assertions only?
                presence:       RwLock::new(Vec::new()),
                tx:             Mutex::new(Transmitter::new_blackhole(slab_id)),
                return_address: RwLock::new(TransportAddress::Blackhole),
            };

            slabref = SlabRef(Arc::new(inner));
            self.peer_refs.write().expect("peer_refs.write()").push(slabref.clone());
        }

        if slab_id == slabref.owning_slab_id {
            return slabref; // no funny business. You don't get to tell me how to reach me
        }

        for p in presence.iter(){
            assert!(slab_id == p.slab_id, "presence slab_id does not match the provided slab_id");

            let mut _maybe_slab = None;
            let args = if p.address.is_local() {
                // playing silly games with borrow lifetimes.
                // TODO: make this less ugly
                _maybe_slab = self.net.get_slab(p.slab_id);

                if let Some(ref slab) = _maybe_slab {
                    TransmitterArgs::Local(slab)
                }else{
                    continue;
                }
            }else{
                TransmitterArgs::Remote( &p.slab_id, &p.address )
            };
             // Returns true if this presence is new to the slabref
             // False if we've seen this presence already

            if slabref.apply_presence(p) {

                let new_trans = self.net.get_transmitter( &args ).expect("assert_slabref net.get_transmitter");
                let return_address = self.net.get_return_address( &p.address ).expect("return address not found");

                *slabref.0.tx.lock().expect("tx.lock()") = new_trans;
                *slabref.0.return_address.write().expect("return_address write lock") = return_address;
            }
        }

        return slabref;

    }
}
