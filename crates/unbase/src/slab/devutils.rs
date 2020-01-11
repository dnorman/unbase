use super::*;

impl fmt::Debug for Slab {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Slab")
            .field("slab_id", &self.id)
            .field("peer_refs", &self.peer_refs)
            .field("memo_refs", &self.memorefs_by_id)
            .finish()
    }
}

impl Slab {
    // Counters,stats, reporting
    pub fn count_of_memorefs_resident( &self ) -> u32 {
        self.memorefs_by_id.read().unwrap().len() as u32
    }
    pub fn count_of_memos_received( &self ) -> u64 {
        self.counters.read().unwrap().memos_received as u64
    }
    pub fn count_of_memos_reduntantly_received( &self ) -> u64 {
        self.counters.read().unwrap().memos_redundantly_received as u64
    }
    pub fn peer_slab_count (&self) -> usize {
        self.peer_refs.read().unwrap().len() as usize
    }
}
