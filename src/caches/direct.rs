
use crate::caches::cachemeta::CacheMeta;
use crate::Cache;
use crate::caches::cache::AnyCache;

pub struct DirectCache {
        offset_bits: u32,
        index_bits: u32,
        index_mask: u64,
        line_size: u64,

        // Cache storage
        // No need for valid bits or dirty bits since we are only simulating reads
        
        // Store tags as a vector of u64s, where each u64 represents the tag of a cache line
        // Option used to represent valid/invalid lines
        tags: Vec<Option<u64>>,

        hits: u64,
        misses: u64,
        main_memory_accesses: u64,

        // Store a reference to lower level cache if exists
        lower: Option<Box<AnyCache>>,
}

impl Cache for DirectCache {
        fn new(meta: CacheMeta) -> Self {
                // Calculate no of cache lines
                let num_lines = (meta.size / meta.line_size) as u64;
                let line_size = meta.line_size;

                // Calculate bits for offset, index, and tag
                let offset_bits = meta.line_size.trailing_zeros();
                let index_bits = num_lines.trailing_zeros();

                let index_mask = (1u64 << index_bits) - 1;

                // Initialise cache with empty lines
                DirectCache {
                        offset_bits,
                        index_bits,
                        index_mask,
                        line_size,
                        tags: vec![None; num_lines as usize],
                        hits: 0,
                        misses: 0,
                        main_memory_accesses: 0,
                        lower: None,
                }
        }

        
        // "Perform access"
        fn access(&mut self, address: u64, size: u64) {
                // Calculate which cache lines are accessed (& check for accesses that span across lines)
                // end_line has -1 since if we access the last byte of a line, it should still count as accessing that line
                let start_line = address >> self.offset_bits;
                let end_line = (address + size - 1) >> self.offset_bits; 

                // Access each cache line that the address + size spans
                for line_index in start_line..=end_line {
                        let line_address = line_index * self.line_size;
                        let index = self.get_index(line_address) as usize;
                        let tag = self.get_tag(line_address);

                        // check for hit
                        if self.tags[index] == Some(tag) {
                                // HIT
                                self.hits += 1;
                        } else {
                                // MISS

                                // we're just simulating reads, so can just overwrite lines on miss without worrying about evicting dirty lines
                                self.tags[index] = Some(tag);
                                self.misses += 1;

                                // If we have a lower level cache, access it as well
                                // if not - "Access main memory"
                                if let Some(lower_cache) = &mut self.lower {
                                        // fetch entire cache line
                                        lower_cache.access(line_address, self.line_size);
                                } else {
                                        self.main_memory_accesses += 1;
                                }

                        }
                }

        }


        // Recursively get metadata on hits and misses from this cache and all lower caches
        // Format into vec
        fn get_hits(&self, mut hits_vec: Vec<u64>) -> Vec<u64> {

                hits_vec.push(self.hits);

                if let Some(lower_cache) = &self.lower {
                        hits_vec = lower_cache.get_hits(hits_vec);
                }

                hits_vec
        }

        fn get_misses(&self, mut misses_vec: Vec<u64>) -> Vec<u64> {

                misses_vec.push(self.misses);

                if let Some(lower_cache) = &self.lower {
                        misses_vec = lower_cache.get_misses(misses_vec);
                }

                misses_vec
        }

        // Get main mem accesses from lowest cache
        fn get_main_memory_accesses(&self) -> u64 {
                if let Some(lower_cache) = &self.lower {
                        lower_cache.get_main_memory_accesses()
                } else {
                        self.main_memory_accesses
                }
        }

        // Set reference to lower level cache
        fn set_lower(&mut self, lower_cache: Box<AnyCache>) {
                self.lower = Some(lower_cache);
        }
}

impl DirectCache {

        // extract tag from address
        #[inline(always)]
        fn get_tag(&self, address: u64) -> u64 {
                address >> (self.offset_bits + self.index_bits)
        }


        // extract index from address
        #[inline(always)]
        fn get_index(&self, address: u64) -> u64 {
                (address >> self.offset_bits) & self.index_mask
        }

}