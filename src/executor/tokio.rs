
use tokio::executor::current_thread;

pub struct Executor{}

impl Executor {
    pub fn spawn< B >( future: B ) where
        B: Future< Item = (), Error = () > + 'static {
        current_thread::spawn( future );
    }
}
