
struct Executor{

}

impl Executor{
    pub fn new () -> Self {
        Executor{
            
        }
    }
    pub fn spawn_task < B >( future: B ) where
        B: Future< Item = (), Error = () > + 'static {

    }
}