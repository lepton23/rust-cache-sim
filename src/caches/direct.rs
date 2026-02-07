
use crate::caches::cachemeta::Cachemeta;

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
}

impl DirectCache {
        pub fn new(meta: CacheMeta) -> Self {
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
                        tags: vec![0; num_lines as u64],
                        hits: 0,
                }
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

        // extract offset from address
        fn get_offset(&self, address: u64) -> u64 {
                let mask = (1 << self.offset_bits) - 1;
                address & mask
        }

        
        // perform access
        pub fn access(&mut self, address: u64) {
                let index = self.get_index(address);
                let tag = self.get_tag(address);

                // check for hit
                if self.tags[index] == tag {
                        self.hits += 1;
                        true
                } else {
                        self.tags[index] = tag;
                        false
                }
        }


        pub fn get_hits(&self) -> u64 {
                self.hits
        }

}