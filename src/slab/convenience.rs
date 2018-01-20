use super::*;

impl Slab {
    pub fn new_memo_basic (&self, subject_id: Option<SubjectId>, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        self.new_memo(subject_id, parents, body)
    }
    pub fn new_memo_basic_noparent (&self, subject_id: Option<SubjectId>, body: MemoBody) -> MemoRef {
        self.new_memo(subject_id, MemoRefHead::Null, body)
    }
    // should this be a function of the slabref rather than the owning slab?
    pub fn presence_for_origin (&self, origin_slabref: &SlabRef ) -> SlabPresence {
        // Get the address that the remote slab would recogize
        SlabPresence {
            slab_id: self.id,
            address: origin_slabref.get_return_address(),
            lifetime: SlabAnticipatedLifetime::Unknown
        }
    }
    pub fn slabref_from_local_slab(&self, peer_slab: &Self) -> SlabRef {

        //let args = TransmitterArgs::Local(&peer_slab);
        let presence = SlabPresence{
            slab_id: peer_slab.id,
            address: TransportAddress::Local,
            lifetime: SlabAnticipatedLifetime::Unknown
        };

        self.assert_slabref(peer_slab.id, &vec![presence])
    }
    pub fn slabref_from_presence(&self, presence: &SlabPresence) -> Result<SlabRef,&str> {
            match presence.address {
                TransportAddress::Simulator  => {
                    return Err("Invalid - Cannot create simulator slabref from presence")
                }
                TransportAddress::Local      => {
                    return Err("Invalid - Cannot create local slabref from presence")
                }
                _ => { }
            };


        //let args = TransmitterArgs::Remote( &presence.slab_id, &presence.address );

        Ok(self.assert_slabref( presence.slab_id, &vec![presence.clone()] ))
    }
}
