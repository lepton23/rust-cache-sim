
use crate::caches::cachemeta::CacheMeta;
use crate::Cache;

use std::collections::HashMap;


pub struct FullAssocCache {
        meta: CacheMeta,
        offset_bits: u32,
        num_lines: usize,

        // Cache storage
        
        // Store hashmap of tags to their index in the cache
        tag_map: HashMap<u64, usize>,

        // For replacement policies track order & frequency
        rr_counter: usize, // for RR
        access_order: Vec<u64>, // for LRU
        access_frequency: HashMap<u64, u64>, // for LFU

        hits: u64,
        misses: u64,
        main_memory_accesses: u64,

        // Store a reference to lower level cache if exists
        lower: Option<Box<dyn Cache>>,

        rp: u8, // replacement policy - 0 = RR, 1 = LRU, 2 = LFU
}

impl Cache for FullAssocCache {
        fn new(meta: CacheMeta) -> Self {
                // Calculate no of cache lines
                let num_lines = (meta.size / meta.line_size) as usize;

                // Calculate bits for offset, index, and tag
                let offset_bits = meta.line_size.trailing_zeros();

                // Initialise cache
                FullAssocCache {
                        meta: meta.clone(),
                        offset_bits,
                        num_lines,
                        tag_map: HashMap::with_capacity(num_lines),
                        rr_counter: 0,
                        access_order: Vec::with_capacity(num_lines),
                        access_frequency: HashMap::with_capacity(num_lines),
                        hits: 0,
                        misses: 0,
                        main_memory_accesses: 0,
                        lower: None,
                        rp: meta.rp,
                }
        }

        
        // "Perform access"
        fn access(&mut self, address: u64, size: u64) {
                // Calculate which cache lines are accessed (& check for accesses that span across lines)
                // end_line has -1 since if we access the last byte of a line, it should still count as accessing that line
                let start_line = address / self.meta.line_size;
                let end_line = (address + size - 1) / self.meta.line_size; 

                // Access each cache line that the address + size spans
                for line_index in start_line..=end_line {
                        let line_address = line_index * self.meta.line_size;
                        let tag = self.get_tag(line_address);

                        // Check for hit
                        if self.tag_map.contains_key(&tag) {
                                // HIT
                                self.hits += 1;

                                if self.rp == 1 {
                                        // Update access order for LRU
                                        self.access_order.retain(|&t| t != tag); // remove tag from current position
                                        self.access_order.push(tag); // add tag to the end to mark as most recently used
                                }
                                if self.rp == 2 {
                                        // Update access frequency for LFU
                                        *self.access_frequency.entry(tag).or_insert(0) += 1;
                                }
                        } else {
                                // MISS
                                self.misses += 1;

                                // If cache not full just insert at next available index
                                if self.tag_map.len() < self.num_lines {
                                        let free_index = (0..self.num_lines)
                                                .find(|i| !self.tag_map.values().any(|&index| index == *i))
                                                .expect("No free index found");

                                        self.tag_map.insert(tag, free_index);

                                        if self.rp == 1 {
                                                // update access order for LRU
                                                self.access_order.push(tag);
                                        }
                                        else if self.rp == 2 {
                                                // Initialise frequency for new tag for LFU
                                                self.access_frequency.insert(tag, 0);
                                        }

                                // CACHE FULL
                                } else {
                                        // Cache full, need to evict line
                                        let (victim_tag, victim_index) = match self.rp {
                                                0 => {
                                                        let idx = self.get_rr_index();
                                                        let vtag = *self.tag_map.iter()
                                                                .find(|&(_, &index)| index == idx)
                                                                .map(|(tag, _)| tag)
                                                                .expect("VTag not found for RR");
                                                        (vtag, idx)
                                                },
                                                1 => {
                                                        // least recently used tag at start of vec
                                                        let vtag = self.access_order.remove(0); 
                                                        let idx = *self.tag_map.get(&vtag)
                                                                .expect("VTag not found for LRU");
                                                        (vtag, idx)
                                                },
                                                2 => {
                                                        // least frequently used tag tiebroken by lowest index
                                                        // tiebreak ensures deterministic behaviour
                                                        let vtag = self.get_lfu_tag();
                                                        let idx = *self.tag_map.get(&vtag)
                                                                .expect("VTag nbot found for LFU");
                                                        (vtag, idx)
                                                },
                                                _ => panic!("Invalid replacement policy"),
                                        };

                                        // Remove victim from all tracking structs

                                        self.tag_map.remove(&victim_tag);
                                        if self.rp == 1 {
                                                self.access_order.retain(|&t| t != victim_tag);
                                        }
                                        else if self.rp == 2 {
                                                self.access_frequency.remove(&victim_tag);
                                        }

                                        // Insert new tag
                                        self.tag_map.insert(tag, victim_index);
                                        if self.rp == 1 {
                                                self.access_order.push(tag);
                                        }
                                        else if self.rp == 2 {
                                                self.access_frequency.insert(tag, 0);
                                        }
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

        // Recursively get metadata on hits and misses from this cache and all lower caches
        // Format into vec
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

        // Get main mem accesses from lowest cache
        fn get_main_memory_accesses(&self) -> u64 {
                if let Some(lower_cache) = &self.lower {
                        lower_cache.get_main_memory_accesses()
                } else {
                        self.main_memory_accesses
                }
        }

        // Set reference to lower level cache
        fn set_lower(&mut self, lower_cache: Box<dyn Cache>) {
                self.lower = Some(lower_cache);
        }
}

impl FullAssocCache {

        // extract tag from address
        fn get_tag(&self, address: u64) -> u64 {
                address >> (self.offset_bits)
        }

        fn get_rr_index(&mut self) -> usize {
                // find the first invalid line
                let index = self.rr_counter % self.num_lines;
                self.rr_counter += 1;
                index
        }

        fn get_lfu_tag(&mut self) -> u64 {
                // least frequently used line has smallest access frequency
                // use lower cache index as tiebreaker to ensure deterministic behaviour
                self.access_frequency.iter()
                        .min_by_key(|&(&tag, &freq)| {
                                let index = *self.tag_map.get(&tag).expect("LFU tag not found");
                                (freq, index)
                        })
                        .map(|(&tag, _)| tag)
                        .expect("No LFU tags found")
        }
}