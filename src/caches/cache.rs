use crate::caches::cachemeta::CacheMeta;

pub trait Cache {
        fn new(meta: CacheMeta) -> Self where Self: Sized;
        fn access(&mut self, address: u64, size: u64) -> ();
        fn get_hits(&self, hits_vec: Vec<u64>) -> Vec<u64>;
        fn get_misses(&self, misses_vec: Vec<u64>) -> Vec<u64>;
        fn get_main_memory_accesses(&self) -> u64;
        fn set_lower(&mut self, lower_cache: Box<dyn Cache>);
}