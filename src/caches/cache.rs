pub trait Cache {
        fn access(&mut self, address: u64, size: u64);
        fn access_line(&mut self, line_index: u64);
        fn get_hits(&self, hits_vec: Vec<u64>) -> Vec<u64>;
        fn get_misses(&self, misses_vec: Vec<u64>) -> Vec<u64>;
        fn get_main_memory_accesses(&self) -> u64;
}

// Terminator for monomorphised cache hierarchy
pub struct MainMemory {
        pub accesses: u64,
}

impl MainMemory {
        pub fn new() -> Self {
                MainMemory { accesses: 0 }
        }
}

impl Cache for MainMemory {
        fn access(&mut self, _address: u64, _size: u64) {
                self.accesses += 1;
        }

        fn access_line(&mut self, _line_index: u64) {
                self.accesses += 1;
        }

        fn get_hits(&self, hits_vec: Vec<u64>) -> Vec<u64> {
                hits_vec
        }

        fn get_misses(&self, misses_vec: Vec<u64>) -> Vec<u64> {
                misses_vec
        }

        fn get_main_memory_accesses(&self) -> u64 {
                self.accesses
        }
}

