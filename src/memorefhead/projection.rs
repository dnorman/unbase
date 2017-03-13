use super::*;

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
    pub fn project_all_relation_links (&self, slab: &Slab) -> &[SubjectId] {
        let mut relation_links : [RelationSlotId; SUBJECT_MAX_RELATIONS] = [0; SUBJECT_MAX_RELATIONS];

        // TODO: how to handle relationship nullification?
        for memo in self.causal_memo_iter(slab){
            match memo.inner.body {
                MemoBody::FullyMaterialized { v: _, ref r } => {

                    for (slot,&(subject_id,_)) in r {
                        relation_links[ slot ] = subject_id;
                    }
                    break;
                    // Materialized memo means we're done here
                },
                MemoBody::Relation(ref r) => {
                    for (slot,&(subject_id,_)) in r.iter() {
                        relation_links[ slot ] = subject_id;
                    }
                },
                _ => {}
            }
        }

        relation_links

    }

    pub fn project_value ( &self, context: &Context, key: &str ) -> Option<String> {

        //TODO: consider creating a consolidated projection routine for most/all uses
        for memo in self.causal_memo_iter(context.get_slab()) {

            println!("# \t\\ Considering Memo {}", memo.id );
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
    pub fn project_relation ( &self, context: &Context, key: u8 ) -> Result<&(SubjectId,Self), RetrieveError> {
        // TODO: Make error handling more robust

        for memo in self.causal_memo_iter( context.get_slab() ) {

            if let Some((relations,materialized)) = memo.get_relations(){
                println!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}", memo.id, memo.get_parent_head(), relations );
                if let Some(r) = relations.get(&key) {
                    // BUG: the parent->child was formed prior to the revision of the child.
                    // TODO: Should be adding the new head memo to the query context
                    //       and superseding the referenced head due to its inclusion in the context

                    return Ok(r);
                }else if materialized {
                    println!("\n# \t\\ Not Found (materialized)" );
                    return Err(RetrieveError::NotFound);
                }
            }
        }

        println!("\n# \t\\ Not Found" );
        Err(RetrieveError::NotFound)
    }

}
