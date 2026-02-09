
use crate::caches::cachemeta::CacheMeta;
use crate::Cache;


pub struct DirectCache {
        meta: CacheMeta,
        offset_bits: u32,
        index_bits: u32,

        // cache storage
        // no need for valid bits or dirty bits since we are only simulating reads
        
        // store tags as a vector of u64s, where each u64 represents the tag of a cache line
        tags: Vec<u64>,

        hits: u64,
        misses: u64,
        main_memory_accesses: u64,

        overflow_accesses: u64, // count accesses that span multiple lines for profiling

        // store a reference to lower level cache if exists
        lower: Option<Box<dyn Cache>>,
}

impl Cache for DirectCache {
        fn new(meta: CacheMeta) -> Self {
                // calculate no of cache lines
                let num_lines = (meta.size / meta.line_size) as u64;

                // calculate bits for offset, index, and tag
                let offset_bits = meta.line_size.trailing_zeros();
                let index_bits = num_lines.trailing_zeros();
                let tag_bits = 64 - offset_bits - index_bits;

                println!("\nInitializing {} cache with size={} bytes, line size={} bytes, num lines={}, offset bits={}, index bits={}, tag bits={}\n", 
                        meta.name, meta.size, meta.line_size, num_lines, offset_bits, index_bits, tag_bits);

                DirectCache {
                        meta,
                        offset_bits,
                        index_bits,
                        tags: vec![0; num_lines as usize],
                        hits: 0,
                        misses: 0,
                        main_memory_accesses: 0,
                        lower: None,
                        overflow_accesses: 0,
                }
        }

        
        // perform access
        fn access(&mut self, address: u64, size: u64) {
                // calculate which cache lines are accessed (i.e. accesses that span across lines)
                // end_line -1 since if we access the last byte of a line, it should still count as accessing that line
                let start_line = address / self.meta.line_size;
                let end_line = (address + size - 1) / self.meta.line_size; 

                // access each cache line that the address spans
                for line_index in start_line..=end_line {
                        let line_address = line_index * self.meta.line_size;
                        let index = self.get_index(line_address) as usize;
                        let tag = self.get_tag(line_address);

                        // check for hit
                        if self.tags[index] == tag {
                                self.hits += 1;
                        } else {
                                self.misses += 1;
                                if end_line > start_line {
                                        self.overflow_accesses += 1;
                                }

                                // we're just simulating reads, so can just overwrite lines on miss without worrying about evicting dirty lines
                                self.tags[index] = tag;

                                // if we have a lower level cache, access it as well
                                // fetch entire cache line
                                if let Some(lower_cache) = &mut self.lower {
                                        // println!("--- Checking lower cache ---");
                                        lower_cache.access(line_address, self.meta.line_size);
                                } else {
                                        self.main_memory_accesses += 1;
                                }

                        }
                }

        }


        fn get_hits(&self, mut hits_vec: Vec<u64>) -> Vec<u64> {

                hits_vec.push(self.hits);

                if let Some(lower_cache) = &self.lower {
                        hits_vec = lower_cache.get_hits(hits_vec.clone());
                }

                hits_vec
        }

        fn get_misses(&self, mut misses_vec: Vec<u64>) -> Vec<u64> {

                misses_vec.push(self.misses);

                if let Some(lower_cache) = &self.lower {
                        misses_vec = lower_cache.get_misses(misses_vec.clone());
                }

                misses_vec
        }

        fn get_main_memory_accesses(&self) -> u64 {
                if let Some(lower_cache) = &self.lower {
                        lower_cache.get_main_memory_accesses()
                } else {
                        self.main_memory_accesses
                }
        }

        fn get_overflow_accesses(&self) -> u64 {
                if let Some(lower_cache) = &self.lower {
                        lower_cache.get_overflow_accesses() + self.overflow_accesses
                } else {
                        self.overflow_accesses
                }
        }

        fn get_meta(&self) -> &CacheMeta {
                &self.meta
        } 
}

impl DirectCache {

        // method to set lower level cache
        pub fn set_lower(&mut self, lower_cache: Box<dyn Cache>) {
                self.lower = Some(lower_cache);
        }

        // extract tag from address
        fn get_tag(&self, address: u64) -> u64 {
                address >> (self.offset_bits + self.index_bits)
        }


        // extract index from address
        fn get_index(&self, address: u64) -> u64 {
                let mask = (1 << self.index_bits) - 1;
                (address >> self.offset_bits) & mask
        }

}