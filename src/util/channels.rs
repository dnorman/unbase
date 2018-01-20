// TODO: How do we create a wrapper around channels which permits static dispatch,
//       but also allows the simulator to be mindful of work in progress?

// Something like WaitChannel

trait RunMode {

}
struct RunModeSimulated{}
struct RunModeLive {}

impl RunMode for RunModeSimulated {}
impl RunMode for RunModeLive {}


struct WaitChannel<T> {

}

impl WaitChannel<RunModeSimulated> {
    pub fn send() {
        
    }
}