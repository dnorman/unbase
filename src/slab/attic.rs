



*** FROM MemoRef: ***
    pub fn exceeds_min_durability_threshold (&self) -> bool {
        let peerlist = self.peerlist.read().unwrap();
        // TODO - implement proper durability estimation logic
        return peerlist.len() > 0
    }
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


    pub fn residentize_memoref(&self, memoref: &MemoRef, memo: Memo) -> bool {
        //println!("# Slab({}).MemoRef({}).residentize()", self.slab_id, memoref.id);

        assert!(memoref.owning_slab_id == self.slab_id);
        assert!( memoref.memo_id() == memo.id );

        // TODO get rid of ptr, and possibly the whole function
        let mut ptr = memoref.ptr.write().unwrap();

        if let MemoRefPtr::Remote = *ptr {
            *ptr = MemoRefPtr::Resident( Arc::new(memo) );

            // should this be using do_peering_for_memo?
            // doing it manually for now, because I think we might only want to do
            // a concise update to reflect our peering status change

            let peering_memoref = self.new_memo(
                None,
                memoref.to_head(),
                MemoBody::Peering(
                    memoref.memo_id(),
                    memoref.subject_id,
                    vec![ MemoPeerState{
                        slabref: self.slabref.clone(),
                        status: MemoPeerStatus::Resident
                    }]
                )
            );

            //TODO1
            unimplemented!();
            // let requests = Vec::new();
            // for peer in memoref.peerstate.read().unwrap().iter() {

            //     requests.push( self.call(LocalSlabRequest::SendMemo{ slabref: peer.slab_id, memoref: peering_memoref.clone() } ) );
            //     peer.slabref.send( &self.slabref, &peering_memoref );
            // }

            // residentized
            true
        }else{
            // already resident
            false
        }
    }


        // pub fn remotize_memo_ids_wait( &self, memo_ids: &[MemoId], ms: u64 ) -> Result<(),Error> {
    //     use std::time::{Instant,Duration};
    //     let start = Instant::now();
    //     let wait = Duration::from_millis(ms);
    //     use std::thread;

    //     loop {
    //         if start.elapsed() > wait{
    //             return Err(Error::StorageOpDeclined(StorageOpDeclined::InsufficientPeering))
    //         }

    //         #[allow(unreachable_patterns)]
    //         match self.call(LocalSlabRequest::RemotizeMemoIds{ memo_ids } ).wait() {
    //             Ok(_) => {
    //                 return Ok(())
    //             },
    //             Err(Error::StorageOpDeclined(StorageOpDeclined::InsufficientPeering)) => {}
    //             Err(e)                                      => return Err(e)
    //         }

    //         thread::sleep(Duration::from_millis(50));
    //     }
    // }

    // pub fn slabhandle_from_presence(&self, presence: &SlabPresence) -> Result<SlabHandle,Error> {
    //         match presence.address {
    //             TransportAddress::Simulator | TransportAddress::Local  => {
    //                 return Err(Error::StorageOpDeclined(StorageOpDeclined::InvalidAddress))
    //             }
    //             _ => { }
    //         };


    //     //let args = TransmitterArgs::Remote( &presence.slab_id, &presence.address );
    //     presence.get_transmitter(&self.net);

    //     Ok(self.put_slabref( presence.slab_id, &vec![presence.clone()] ))
    // }



        //             if slab.request_memo(self) > 0 {
        //         channel = slab.memo_wait_channel(self.slab_id);
        //     }else{
        //         return Err(Error::RetrieveError(RetrieveError::NotFound))
        //     }

        // // By sending the memo itself through the channel
        // // we guarantee that there's no funny business with request / remotize timing


        // use std::time;
        // let timeout = time::Duration::from_millis(100000);

        // for _ in 0..3 {
        //     match channel.recv_timeout(timeout) {
        //         Ok(memo)       =>{
        //             //println!("Slab({}).MemoRef({}).get_memo() received memo: {}", self.owning_slab_id, self.slab_id, memo.id );
        //             return Ok(memo)
        //         }
        //         Err(rcv_error) => {

        //             use std::sync::mpsc::RecvTimeoutError::*;
        //             match rcv_error {
        //                 Timeout => {}
        //                 Disconnected => {
        //                     return Err(Error::RetrieveError(RetrieveError::SlabError))
        //                 }
        //             }
        //         }
        //     }

        //     // have another go around
        //     if slab.request_memo( &self ) == 0 {
        //         return Err(Error::RetrieveError(RetrieveError::NotFound))
        //     }

        // }

        // Err(Error::RetrieveError(RetrieveError::NotFoundByDeadline))
