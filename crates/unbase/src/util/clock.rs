
const MAX_READING_WIDTH;

// TODO - beacon clock implementation

struct BeaconClock {
    stash: Stash // Veeeery similar to an index stash (In what ways is it different?)
}

impl BeaconClock{
    pub fn probablistic_ping (){

    }

    pub fn get_reading(&self) -> Head {
        // Compress stash to MAX_READING_WIDTH
        // return Head
    }

    /// Used for biasing freshness / liveness
    pub fn compare_min_ticks (&self, head: Head, limit: u32 ) -> u32 {
        // Take a reading now ( or actually just use the whole stash )
        // return the minimum number of clock ticks which have elapsed between the two readings
        // Give up after limit ticks traversed

        // A A A        / A
        //       \ B B     (Six ticks) elapsed
    }
}