use crate::caches::cachemeta::CacheMeta;

pub trait Cache {
        fn new(meta: CacheMeta) -> Self where Self: Sized;
        fn access(&mut self, address: u64, size: u8) -> bool;
        fn get_hits(&self) -> u64;
        fn get_meta(&self) -> &CacheMeta;
}