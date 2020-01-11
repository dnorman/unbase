use super::*;

impl Slab {
    pub fn consider_emit_memo(&self, memoref: &MemoRef) {
        // Emit memos for durability and notification purposes
        // At present, some memos like peering and slab presence are emitted manually.
        // TODO: This will almost certainly have to change once gossip/plumtree functionality is added

        // TODO: test each memo for durability_score and emit accordingly
        if let Some(memo) = memoref.get_memo_if_resident() {
            let needs_peers = self.check_peering_target(&memo);

            //println!("Slab({}).consider_emit_memo {} - A ({:?})", self.id, memoref.id, &*self.peer_refs.read().unwrap() );
            for peer_ref in self.peer_refs.read().unwrap().iter().filter(|x| !memoref.is_peered_with_slabref(x) ).take( needs_peers as usize ) {

                //println!("# Slab({}).emit_memos - EMIT Memo {} to Slab {}", self.id, memo.id, peer_ref.slab_id );
                peer_ref.send( &self.my_ref, memoref );
            }
        }
    }

}
