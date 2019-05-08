use super::*;
use crate::slab::slabref::serde::*;
use crate::util::serde::*;

impl StatefulSerialize for MemoPeerList {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {

        let mut seq = serializer.serialize_seq(None)?;
        for memopeer in self.iter() {
            // don't tell the receiving slab that they have it.
            // They know they have it
            if &memopeer.slabref.slab_id != helper.dest_slab_id {
                seq.serialize_element(&SerializeWrapper(memopeer,helper))?
            }
        }
        seq.end()
    }
}

//             [      [  [[]],     "Resident" ]  ]
// MemoPeerList^  Peer^  ^Slabref  ^Status

impl StatefulSerialize for MemoRef {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        use super::MemoRefPtr::*;

        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element(&self.id)?;
        seq.serialize_element(&self.subject_id)?;
        seq.serialize_element(&match &*self.ptr.read().unwrap() {
            &Remote      => false,
            &Resident(_) => true
        })?;

        // QUESTION: Should we be using memoref.get_peerlist_for_peer instead of has_memo?
        //           What about relayed memos which Slab A requests from B but actually receives from C?
        seq.serialize_element( &SerializeWrapper(&*self.peerlist.read().unwrap(), helper) )?;
        seq.end()
    }
}

/* impl StatefulSerialize for MemoRef {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let shared = &self.shared.lock().unwrap();

        let mut sv = serializer.serialize_struct("Memoref", 4)?;
        sv.serialize_field("memo_id",    &self.id)?;
        sv.serialize_field("subject_id", &self.subject_id )?;

        use super::MemoRefPtr::*;

        sv.serialize_field("resident", &match &shared.ptr {
            &Remote      => false,
            &Resident(_) => true
        })?;

        sv.serialize_field("peers", &SerializeWrapper(&shared.peers, helper) )?;
        sv.end()

    }
}*/

impl StatefulSerialize for MemoPeer {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element(&SerializeWrapper(&self.slabref, helper))?;
        seq.serialize_element(&self.status)?;
        seq.end()
    }
}

#[derive(Clone)]
pub struct MemoRefSeed<'a> { pub dest_slab: &'a Slab, pub origin_slabref: &'a SlabRef }

impl<'a> DeserializeSeed for MemoRefSeed<'a> {
    type Value = MemoRef;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'a> Visitor for MemoRefSeed<'a> {
    type Value = MemoRef;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
       formatter.write_str("struct MemoRef")
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<MemoRef, V::Error>
       where V: SeqAccess
    {
        let memo_id: MemoId = match visitor.visit()? {
            Some(value) => value,
            None => {
                return Err(DeError::invalid_length(0, &self));
            }
        };
        let subject_id: Option<SubjectId> = match visitor.visit()? {
           Some(value) => value,
           None => {
               return Err(DeError::invalid_length(1, &self));
           }
        };
        let has_memo: bool = match visitor.visit()? {
           Some(value) => value,
           None => {
               return Err(DeError::invalid_length(2, &self));
           }
        };

        let mut peers: Vec<MemoPeer> = match visitor.visit_seed( VecSeed( MemoPeerSeed{ dest_slab: self.dest_slab } ) )? {
           Some(value) => value,
           None => {
               return Err(DeError::invalid_length(3, &self));
           }
        };

        peers.push(MemoPeer{
            slabref: self.origin_slabref.clone(),
            status: if has_memo {
                MemoPeeringStatus::Resident
            } else {
                MemoPeeringStatus::Participating
            }
        });

        Ok(self.dest_slab.assert_memoref(memo_id, subject_id, MemoPeerList::new(peers), None).0 )
    }
}

#[derive(Clone)]
pub struct MemoPeerSeed<'a> { pub dest_slab: &'a Slab }

impl<'a> DeserializeSeed for MemoPeerSeed<'a> {
    type Value = MemoPeer;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'a> Visitor for MemoPeerSeed<'a> {
    type Value = MemoPeer;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
       formatter.write_str("struct MemoPeer")
    }
    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
       where V: SeqAccess
    {
        let slabref: SlabRef = match visitor.visit_seed( SlabRefSeed{ dest_slab: self.dest_slab })? {
            Some(value) => value,
            None => {
                return Err(DeError::invalid_length(0, &self));
            }
        };
        let status: MemoPeeringStatus = match visitor.visit()? {
           Some(value) => value,
           None => {
               return Err(DeError::invalid_length(1, &self));
           }
        };

       Ok(MemoPeer{
           slabref: slabref,
           status: status
       })
    }
}
