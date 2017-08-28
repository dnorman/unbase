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
    pub fn project_all_edge_links_including_empties (&self, slab: &Slab) -> Vec<EdgeLink> {

        //let mut edge_links : [Option<EdgeLink>; SUBJECT_MAX_RELATIONS];// = [None; SUBJECT_MAX_RELATIONS];
        let mut edge_links : Vec<Option<EdgeLink>> = Vec::with_capacity(255);

        // None is an indication that we've not yet visited this slot, and that it is thus eligible for setting
        for i in 0..SUBJECT_MAX_RELATIONS as usize {
            edge_links[i] = None;
        }

        for memo in self.causal_memo_iter(slab){
            match memo.body {
                MemoBody::FullyMaterialized { e : ref edgeset, .. } => {

                    // Iterate over all the entries in this EdgeSet
                    for (slot_id,rel_head) in &edgeset.0 {

                        // Only consider the non-visited slots
                        if let None = edge_links[ *slot_id as usize ] {
                            edge_links[ *slot_id as usize ] = Some(match *rel_head {
                                MemoRefHead::None  => EdgeLink::Vacant{ slot_id: *slot_id },
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
                                    MemoRefHead::None  => EdgeLink::Vacant{ slot_id: *slot_id },
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
    /// Project all relation links which were edited between two MemoRefHeads.
    pub fn project_edge_links(&self, reference_head: Option<MemoRefHead>, head: MemoRefHead ) -> Vec<EdgeLink>{
        unimplemented!()
    }
    pub fn project_value ( &self, context: &Context, key: &str ) -> Option<String> {

        //TODO: consider creating a consolidated projection routine for most/all uses
        for memo in self.causal_memo_iter(&context.slab) {

            //println!("# \t\\ Considering Memo {}", memo.id );
            if let Some((values, materialized)) = memo.get_values() {
                if let Some(v) = values.get(key) {
                    return Some(v.clone());
                }else if materialized {
                    return None; //end of the line here
                }
            }
        }
        None
    }
    pub fn project_relation ( &self, context: &Context, key: RelationSlotId ) -> Result<Option<SubjectId>, RetrieveError> {
        // TODO: Make error handling more robust

        for memo in self.causal_memo_iter( &context.slab ) {

            if let Some((relations,materialized)) = memo.get_relations(){
                //println!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}", memo.id, memo.get_parent_head(), relations );
                if let Some(maybe_subject_id) = relations.get(&key) {
                    // BUG: the parent->child was formed prior to the revision of the child.
                    // TODO: Should be adding the new head memo to the query context
                    //       and superseding the referenced head due to its inclusion in the context

                    return Ok(*maybe_subject_id);
                }else if materialized {
                    //println!("\n# \t\\ Not Found (materialized)" );
                    return Err(RetrieveError::NotFound);
                }
            }
        }

        //println!("\n# \t\\ Not Found" );
        Err(RetrieveError::NotFound)
    }    
    pub fn project_edge ( &self, context: &Context, key: RelationSlotId ) -> Result<Option<Self>, RetrieveError> {
        // TODO: Make error handling more robust

        for memo in self.causal_memo_iter( &context.slab ) {

            if let Some((edges,materialized)) = memo.get_edges(){
                //println!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}", memo.id, memo.get_parent_head(), relations );
                if let Some(head) = edges.get(&key) {
                    // BUG: the parent->child was formed prior to the revision of the child.
                    // TODO: Should be adding the new head memo to the query context
                    //       and superseding the referenced head due to its inclusion in the context

                    return Ok(Some(head.clone()));
                }else if materialized {
                    //println!("\n# \t\\ Not Found (materialized)" );
                    return Err(RetrieveError::NotFound);
                }
            }
        }

        //println!("\n# \t\\ Not Found" );
        Err(RetrieveError::NotFound)
    }

}
