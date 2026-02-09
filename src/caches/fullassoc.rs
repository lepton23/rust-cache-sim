
use crate::caches::cachemeta::CacheMeta;
use crate::Cache;

use std::collections::HashMap;


pub struct FullAssocCache {
        meta: CacheMeta,
        offset_bits: u32,
        num_lines: usize,

        // cache storage
        // no need for valid bits or dirty bits since we are only simulating reads
        
        // store hashmap of tags to their index in the cache
        tag_map: HashMap<u64, usize>,

        // for replacement policies track order & frequency
        access_order: Vec<u64>, // for LRU
        access_frequency: HashMap<u64, u64>, // for LFU

        hits: u64,
        misses: u64,
        main_memory_accesses: u64,

        // store a reference to lower level cache if exists
        lower: Option<Box<dyn Cache>>,

        rp: u8, // replacement policy - 0 = RR, 1 = LRU, 2 = LFU
}

impl Cache for FullAssocCache {
        fn new(meta: CacheMeta) -> Self {
                // calculate no of cache lines
                let num_lines = (meta.size / meta.line_size) as u64;

                // calculate bits for offset, index, and tag
                let offset_bits = meta.line_size.trailing_zeros();
                let tag_bits = 64 - offset_bits;

                println!("\nInitializing {} cache with size={} bytes, line size={} bytes, num lines={}, offset bits={}, tag bits={}\n", 
                        meta.name, meta.size, meta.line_size, num_lines, offset_bits, tag_bits);

                FullAssocCache {
                        meta: meta.clone(),
                        offset_bits,
                        num_lines: num_lines as usize,
                        tag_map: HashMap::with_capacity(num_lines as usize),
                        access_order: Vec::with_capacity(num_lines as usize),
                        access_frequency: HashMap::with_capacity(num_lines as usize),
                        hits: 0,
                        misses: 0,
                        main_memory_accesses: 0,
                        lower: None,
                        rp: meta.rp,
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
                        let tag = self.get_tag(line_address);

                        // check for hit
                        if self.tag_map.contains_key(&tag) {
                                self.hits += 1;

                                if self.rp == 1 {
                                        // update access order for LRU
                                        self.access_order.retain(|&t| t != tag); // remove tag from current position
                                        self.access_order.push(tag); // add tag to the end to mark it as most recently used
                                }
                        } else {
                                self.misses += 1;

                                // if cache not full just insert
                                if self.tag_map.len() < self.num_lines {
                                        let index = self.tag_map.len();
                                        self.tag_map.insert(tag, index);
                                        self.access_order.push(tag);
                                        self.access_frequency.insert(tag, 1);
                                } else {
                                        // cache full, need to evict line
                                        let victim_index = match self.rp {
                                                0 => self.get_RR_index(),
                                                1 => self.get_LRU_index(),
                                                2 => self.get_LFU_index(),
                                                _ => panic!("Invalid replacement policy"),
                                        };

                                        self.tag_map.insert(tag, victim_index);
                                        self.access_order.push(tag);
                                        *self.access_frequency.entry(tag).or_insert(0) += 1;
                                }


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

        fn get_meta(&self) -> &CacheMeta {
                &self.meta
        } 

        // method to set lower level cache
        fn set_lower(&mut self, lower_cache: Box<dyn Cache>) {
                self.lower = Some(lower_cache);
        }
}

impl FullAssocCache {

        // extract tag from address
        fn get_tag(&self, address: u64) -> u64 {
                address >> (self.offset_bits)
        }

        fn get_RR_index(&self) -> usize {
                // find the first invalid line
                if self.tag_map.len() < self.num_lines {
                        return self.tag_map.len();
                }
                0
        }

        fn get_LRU_index(&self) -> usize {
                // for LRU, we would need to track usage of lines, which adds complexity
                // for simplicity, we can just use the RR index as a placeholder for LRU eviction
                self.get_RR_index()
        }

        fn get_LFU_index(&self) -> usize {
                // for LFU, we would need to track usage frequency of lines, which adds complexity
                // for simplicity, we can just use the RR index as a placeholder for LFU eviction
                self.get_RR_index()
        }
}