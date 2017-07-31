use super::*;
use std::iter;
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
    pub fn project_all_edge_links_including_empties (&self, slab: &Slab) -> Vec<EdgeLink> {

        let mut edge_links : [Option<EdgeLink>; SUBJECT_MAX_RELATIONS];
        for i in 0..SUBJECT_MAX_RELATIONS as usize {
            edge_links[i] = None;
        }

        // TODO: how to handle relationship nullification?
        for memo in self.causal_memo_iter(slab){
            match memo.body {
                MemoBody::FullyMaterialized { ref e, .. } => {

                    for (slot_id,&rel_head) in &e.0 {

                        // Only consider the non-visited slots
                        if let None = edge_links[ *slot_id as usize ] {
                            edge_links[ *slot_id as usize ] = Some(
                                if subject_id == 0 {
                                    EdgeLink::Vacant{ slot_id: *slot_id }
                                }else{
                                    EdgeLink::Occupied{ slot_id: *slot_id, subject_id, head: rel_head }
                                }
                            );
                        }
                    }
                    break;
                    // Fully Materialized memo means we're done here
                },
                MemoBody::Edge(ref r) => {
                    for (slot_id,maybe_rel_head) in r.iter() {

                        // Only consider the non-visited slots
                        if let None = edge_links[ *slot_id as usize ] {
                            edge_links[ *slot_id as usize ] = Some(
                                match maybe_rel_head {
                                    None               => EdgeLink::Vacant{ slot_id: *slot_id },
                                    Some(ref rel_head @ MemoRefHead::Subject{ subject_id, .. } ) => EdgeLink::Occupied{ slot_id: *slot_id, subject_id, head: rel_head.clone() }
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
            match *maybe_link {
                None       => EdgeLink::Vacant{ slot_id: slot_id as RelationSlotId },
                Some(link) => link
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
    pub fn project_relation ( &self, context: &Context, key: RelationSlotId ) -> Result<SubjectId, RetrieveError> {
        // TODO: Make error handling more robust

        for memo in self.causal_memo_iter( &context.slab ) {

            if let Some((relations,materialized)) = memo.get_relations(){
                //println!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}", memo.id, memo.get_parent_head(), relations );
                if let Some(subject_id) = relations.get(&key) {
                    // BUG: the parent->child was formed prior to the revision of the child.
                    // TODO: Should be adding the new head memo to the query context
                    //       and superseding the referenced head due to its inclusion in the context

                    return Ok(subject_id);
                }else if materialized {
                    //println!("\n# \t\\ Not Found (materialized)" );
                    return Err(RetrieveError::NotFound);
                }
            }
        }

        //println!("\n# \t\\ Not Found" );
        Err(RetrieveError::NotFound)
    }    
    pub fn project_edge ( &self, context: &Context, key: RelationSlotId ) -> Result<(SubjectId,Self), RetrieveError> {
        // TODO: Make error handling more robust

        for memo in self.causal_memo_iter( &context.slab ) {

            if let Some((edges,materialized)) = memo.get_edges(){
                //println!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}", memo.id, memo.get_parent_head(), relations );
                if let Some(&(subject_id, ref head)) = edges.get(&key) {
                    // BUG: the parent->child was formed prior to the revision of the child.
                    // TODO: Should be adding the new head memo to the query context
                    //       and superseding the referenced head due to its inclusion in the context

                    return Ok((subject_id,head.clone()));
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
