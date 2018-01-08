use super::*;
use subject::SUBJECT_MAX_RELATIONS;

impl MemoRefHead {
    /*pub fn fully_materialize( &self, slab: &Slab ) {
        // TODO: consider doing as-you-go distance counting to the nearest materialized memo for each descendent
        //       as part of the list management. That way we won't have to incur the below computational effort.

        for memo in self.causal_memo_iter(slab){
            match memo.inner.body {
                MemoBody::FullyMaterialized { v: _, r: _ } => {},
                _                           => { return false }
            }
        }

        true
    }*/

    // Kind of a brute force way to do this
    // TODO: Consider calculating deltas during memoref application,
    //       and use that to perform a minimum cost subject_head_link edit

    // TODO: This projection method is probably wrong, as it does not consider how to handle concurrent edge-setting
    //       this problem applies to causal_memo_iter itself really, insofar as it should return sets of concurrent memos to be merged rather than individual memos
    // This in turn raises questions about how relations should be merged

    /// Project all edge links based only on the causal history of this head.
    /// The name is pretty gnarly, and this is very ripe for refactoring, but at least it says what it does.
    pub fn project_all_edge_links_including_empties (&self, slab: &Slab) -> Vec<EdgeLink> {

        //let mut edge_links : [Option<EdgeLink>; SUBJECT_MAX_RELATIONS];// = [None; SUBJECT_MAX_RELATIONS];
        let mut edge_links : Vec<Option<EdgeLink>> = Vec::with_capacity(SUBJECT_MAX_RELATIONS);

        // None is an indication that we've not yet visited this slot, and that it is thus eligible for setting
        for _ in 0..SUBJECT_MAX_RELATIONS as usize {
            edge_links.push(None);
        }

        for memo in self.causal_memo_iter(slab){
            let memo = memo.expect("Memo retrieval error. TODO: Update to use Result<..,RetrieveError>");
            match memo.body {
                MemoBody::FullyMaterialized { e : ref edgeset, .. } => {

                    // Iterate over all the entries in this EdgeSet
                    for (slot_id,rel_head) in &edgeset.0 {

                        // Only consider the non-visited slots
                        if let None = edge_links[ *slot_id as usize ] {
                            edge_links[ *slot_id as usize ] = Some(match *rel_head {
                                MemoRefHead::Null  => EdgeLink::Vacant{ slot_id: *slot_id },
                                _                  => EdgeLink::Occupied{ slot_id: *slot_id, head: rel_head.clone() }
                            });
                        }
                    }

                    break;
                    // Fully Materialized memo means we're done here
                },
                MemoBody::Edge(ref r) => {
                    for (slot_id,rel_head) in r.iter() {

                        // Only consider the non-visited slots
                        if let None = edge_links[ *slot_id as usize ] {
                            edge_links[ *slot_id as usize ] = Some(
                                match *rel_head {
                                    MemoRefHead::Null  => EdgeLink::Vacant{ slot_id: *slot_id },
                                    _                  => EdgeLink::Occupied{ slot_id: *slot_id, head: rel_head.clone() }
                                }
                            )
                        }
                    }
                },
                _ => {}
            }
        }

        edge_links.iter().enumerate().map(|(slot_id,maybe_link)| {
            // Fill in the non-visited links with vacants
            match maybe_link {
                &None           => EdgeLink::Vacant{ slot_id: slot_id as RelationSlotId },
                &Some(ref link) => link.clone()
            }
        }).collect()
    }
    /// Contextualized projection of occupied edges
    pub fn project_occupied_edges (&self, slab: &Slab) -> Vec<EdgeLink> {
        let mut visited = [false;SUBJECT_MAX_RELATIONS];
        let mut edge_links : Vec<EdgeLink> = Vec::new();
        
        'memo: for memo in self.causal_memo_iter(&slab){
            let memo = memo.expect("Memo retrieval error. TODO: Update to use Result<..,RetrieveError>");

            let (edgeset,last) = match memo.body {
                MemoBody::FullyMaterialized { e : ref edgeset, .. } => {
                    (edgeset,true)
                },
                MemoBody::Edge(ref edgeset) => {
                    (edgeset,false)
                },
                _ => continue 'memo
            };

            for (slot_id,rel_head) in edgeset.iter() {
                // Only consider the non-visited slots
                if !visited[ *slot_id as usize] {
                    visited[ *slot_id as usize] = true;

                    match *rel_head {
                        MemoRefHead::Subject{..} | MemoRefHead::Anonymous{..} => {
                            edge_links.push( EdgeLink::Occupied{ slot_id: *slot_id, head: rel_head.clone() });
                        },
                        MemoRefHead::Null => {}
                    };
                }
            }
                
            if last {
                break;
            }
        }

        edge_links
    }
                        
    // Fully Materialized memo means we're done here
    /// Project all relation links which were edited between two MemoRefHeads.
    // pub fn project_edge_links(&self, reference_head: Option<MemoRefHead>, head: MemoRefHead ) -> Vec<EdgeLink>{
    //     unimplemented!()
    // }
    pub fn project_value ( &self, slab: &Slab, key: &str ) -> Result<Option<String>,RetrieveError> {

        //TODO: consider creating a consolidated projection routine for most/all uses
        for memo in self.causal_memo_iter(&slab) {
            //println!("# \t\\ Considering Memo {}", memo.id );
            if let Some((values, materialized)) = memo?.get_values() {
                if let Some(v) = values.get(key) {
                    return Ok(Some(v.clone()));
                }else if materialized {
                    return Ok(None); //end of the line here
                }
            }
        }

        Err(RetrieveError::MemoLineageError)
    }
    pub fn project_relation ( &self, slab: &Slab, key: RelationSlotId ) -> Result<Option<SubjectId>, RetrieveError> {

        for memo in self.causal_memo_iter( &slab ) {

            if let Some((relations,materialized)) = memo?.get_relations(){
                //println!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}", memo.id, memo.get_parent_head(), relations );
                if let Some(maybe_subject_id) = relations.get(&key) {
                    return match *maybe_subject_id {
                        Some(subject_id) => Ok(Some(subject_id)),
                        None                 => Ok(None)
                    };
                }else if materialized {
                    //println!("\n# \t\\ Not Found (materialized)" );
                    return Ok(None);
                }
            }
        }

        //println!("\n# \t\\ Not Found" );
        Err(RetrieveError::MemoLineageError)
    }    
    pub fn project_edge ( &self, slab: &Slab, key: RelationSlotId ) -> Result<Option<Self>, RetrieveError> {
        for memo in self.causal_memo_iter( &slab ) {

            if let Some((edges,materialized)) = memo?.get_edges(){
                //println!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}", memo.id, memo.get_parent_head(), relations );
                if let Some(head) = edges.get(&key) {
                    return Ok(Some(head.clone()));
                }else if materialized {
                    //println!("\n# \t\\ Not Found (materialized)" );
                    return Ok(None);
                }
            }
        }

        //println!("\n# \t\\ Not Found" );
        Err(RetrieveError::MemoLineageError)
    }

}
