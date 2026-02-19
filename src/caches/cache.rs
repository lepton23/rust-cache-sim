use crate::caches::cachemeta::CacheMeta;
use super::direct::DirectCache;
use super::n_way::NWayCache;
use super::fullassoc::FullAssocCache;

pub trait Cache {
        fn new(meta: CacheMeta) -> Self where Self: Sized;
        fn access(&mut self, address: u64, size: u64) -> ();
        fn access_line(&mut self, line_index: u64) -> ();
        fn get_hits(&self, hits_vec: Vec<u64>) -> Vec<u64>;
        fn get_misses(&self, misses_vec: Vec<u64>) -> Vec<u64>;
        fn get_main_memory_accesses(&self) -> u64;
        fn set_lower(&mut self, lower_cache: Box<AnyCache>);
}

pub enum AnyCache {
        Direct(DirectCache),
        NWay(NWayCache),
        FullAssoc(FullAssocCache),
}

impl Cache for AnyCache {
        fn new(meta: CacheMeta) -> Self where Self: Sized {
                match meta.kind {
                        0 => AnyCache::Direct(DirectCache::new(meta)),
                        1 => AnyCache::NWay(NWayCache::new(meta)),
                        2 => AnyCache::NWay(NWayCache::new(meta)),
                        3 => AnyCache::NWay(NWayCache::new(meta)),
                        4 => AnyCache::FullAssoc(FullAssocCache::new(meta)),
                        _ => panic!("Unsupported cache type"),
                }
        }

        fn access(&mut self, address: u64, size: u64) -> () {
                match self {
                        AnyCache::Direct(cache) => cache.access(address, size),
                        AnyCache::NWay(cache) => cache.access(address, size),
                        AnyCache::FullAssoc(cache) => cache.access(address, size),
                }
        }

        fn access_line(&mut self, line_index: u64) -> () {
                match self {
                        AnyCache::Direct(cache) => cache.access_line(line_index),
                        AnyCache::NWay(cache) => cache.access_line(line_index),
                        AnyCache::FullAssoc(cache) => cache.access_line(line_index),
                }
        }

        fn get_hits(&self, hits_vec: Vec<u64>) -> Vec<u64> {
                match self {
                        AnyCache::Direct(cache) => cache.get_hits(hits_vec),
                        AnyCache::NWay(cache) => cache.get_hits(hits_vec),
                        AnyCache::FullAssoc(cache) => cache.get_hits(hits_vec),
                }
        }

        fn get_misses(&self, misses_vec: Vec<u64>) -> Vec<u64> {
                match self {
                        AnyCache::Direct(cache) => cache.get_misses(misses_vec),
                        AnyCache::NWay(cache) => cache.get_misses(misses_vec),
                        AnyCache::FullAssoc(cache) => cache.get_misses(misses_vec),
                }
        }

        fn get_main_memory_accesses(&self) -> u64 {
                match self {
                        AnyCache::Direct(cache) => cache.get_main_memory_accesses(),
                        AnyCache::NWay(cache) => cache.get_main_memory_accesses(),
                        AnyCache::FullAssoc(cache) => cache.get_main_memory_accesses(),
                }
        }

        fn set_lower(&mut self, lower_cache: Box<AnyCache>) {
                match self {
                        AnyCache::Direct(cache) => cache.set_lower(lower_cache),
                        AnyCache::NWay(cache) => cache.set_lower(lower_cache),
                        AnyCache::FullAssoc(cache) => cache.set_lower(lower_cache),
                }
        }
}