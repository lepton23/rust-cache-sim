
use crate::caches::cachemeta::CacheMeta;
use crate::Cache;


pub struct DirectCache {
        meta: CacheMeta,
        num_lines: u64,
        offset_bits: u32,
        index_bits: u32,
        tag_bits: u32,

        // cache storage
        // no need for valid bits or dirty bits since we are only simulating reads
        
        // store tags as a vector of u64s, where each u64 represents the tag of a cache line
        tags: Vec<u64>,

        hits: u64,

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

                DirectCache {
                        meta,
                        num_lines,
                        offset_bits,
                        index_bits,
                        tag_bits,
                        tags: vec![0; num_lines as usize],
                        hits: 0,
                        lower: None,
                }
        }

        
        // perform access
        fn access(&mut self, address: u64, size: u8) -> bool {
                // calculate which cache lines are accessed
                let start_line = address / self.meta.line_size;
                let end_line = (address + size as u64 - 1) / self.meta.line_size;

                let mut all_hit = true;

                // access each cache line that the address spans
                for line_addr in start_line..=end_line {
                        let line_address = line_addr * self.meta.line_size;
                        let index = self.get_index(line_address) as usize;
                        let tag = self.get_tag(line_address);

                        // check for hit
                        if self.tags[index] == tag {
                                self.hits += 1;
                        } else {
                                all_hit = false;

                                // we're just simulating reads, so can just overwrite lines on miss without worrying about evicting dirty lines
                                self.tags[index] = tag;

                                // if we have a lower level cache, access it as well
                                if let Some(lower_cache) = &mut self.lower {
                                        lower_cache.access(line_address, self.meta.line_size as u8);
                                }

                        }
                }

                all_hit
        }


        fn get_hits(&self) -> u64 {
                self.hits
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