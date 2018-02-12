
extern crate stdweb;
use stdweb::{Null, Promise, PromiseFuture};

struct Executor{}

impl Executor{
    pub fn spawn_task < B >( future: B ) where
        B: Future< Item = (), Error = () > + 'static {
            PromiseFuture::spawn( future );
    }
}