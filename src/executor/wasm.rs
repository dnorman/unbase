use stdweb::PromiseFuture;
use futures::Future;

pub struct Executor{}

impl Executor {
    pub fn spawn< B >( future: B ) where
        B: Future< Item = (), Error = () > + 'static {
        PromiseFuture::spawn( future );
    }
}
