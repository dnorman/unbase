use super::*;

impl Slab {
    pub fn consider_emit_memo(&self, memoref: &MemoRef) {
        // Emit memos for durability and notification purposes
        // At present, some memos like peering and slab presence are emitted manually.
        // TODO: This will almost certainly have to change once gossip/plumtree functionality is added

        let needs_peers = memoref.want_peer_count();

        if needs_peers > 0 {
            // TODO - get rid of deadlock potential by separating out the remediation queue check somehow
            let mut rem_q = self.peering_remediation_queue.lock().unwrap();
            if !rem_q.contains(&memoref) {
                rem_q.push(memoref.clone());
            }
            //println!("Slab({}).consider_emit_memo {} - A ({:?})", self.id, memoref.id, &*self.peer_refs.read().unwrap() );
            for peer_ref in self.peer_refs.read().unwrap().iter().filter(|x| !memoref.is_peered_with_slabref(x) ).take( needs_peers as usize ) {

                //println!("# Slab({}).emit_memos - EMIT Memo {} to Slab {}", self.id, memoref.id, peer_ref.slab_id );
                peer_ref.send( &self.my_ref, memoref );
            }
        }else{
            let mut q = self.peering_remediation_queue.lock().unwrap();
            q.retain(|mr| mr != memoref )
        }
    }

}
