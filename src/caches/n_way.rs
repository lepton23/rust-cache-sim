
use crate::caches::cachemeta::CacheMeta;
use crate::Cache;

use std::collections::HashMap;

struct CacheSet {
        tags: HashMap<u64, usize>, // tag to index in cache lines
        rr_counter: usize,
        access_order: Vec<u64>,
        access_frequency: HashMap<u64, u64>,
}

impl CacheSet {
        fn new(asso_num: usize) -> Self {
                CacheSet {
                        tags: HashMap::with_capacity(asso_num),
                        rr_counter: 0,
                        access_order: Vec::with_capacity(asso_num),
                        access_frequency: HashMap::with_capacity(asso_num),
                }
        }
}

pub struct NWayCache {
        meta: CacheMeta,
        offset_bits: u32,
        index_bits: u32,
        asso_num: usize,

        // Cache storage //
        // Store vec of sets rather than vec of tags
        // for replacement policies track order & frequency within each set
        sets: Vec<CacheSet>,        

        hits: u64,
        misses: u64,
        main_memory_accesses: u64,

        // Store a reference to lower level cache if exists
        lower: Option<Box<dyn Cache>>,

        rp: u8, // replacement policy - 0 = RR, 1 = LRU, 2 = LFU
}

impl Cache for NWayCache {
        fn new(meta: CacheMeta) -> Self {

                let asso_num: usize = match meta.kind {
                        1 => 2,
                        2 => 4,
                        3 => 8,
                        _ => panic!("Invalid cache kind for NWayCache"),
                };

                // Calculate no. of cache lines & no. of sets
                let num_lines = (meta.size / meta.line_size) as usize;
                let num_sets = num_lines / asso_num;

                // Calculate bits for offset, index, and tag
                let offset_bits = meta.line_size.trailing_zeros();
                let index_bits = (num_sets as u64).trailing_zeros();

                // Initialise sets
                let sets = (0..num_sets)
                        .map(|_| CacheSet::new(asso_num))
                        .collect();

                // Construct cache
                NWayCache {
                        meta: meta.clone(),
                        offset_bits,
                        index_bits,
                        asso_num,
                        sets,
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
                        let set_index = self.get_set_index(line_address);
                        let tag = self.get_tag(line_address);

                        let set = &mut self.sets[set_index];

                        // Check for hit
                        if set.tags.contains_key(&tag) {
                                // HIT
                                self.hits += 1;

                                if self.rp == 1 {
                                        // Update access order for LRU
                                        set.access_order.retain(|&t| t != tag); // remove tag from current position
                                        set.access_order.push(tag); // add tag to the end to mark as most recently used
                                }
                                if self.rp == 2 {
                                        // Update access frequency for LFU
                                        *set.access_frequency.entry(tag).or_insert(0) += 1;
                                }
                        } else {
                                // MISS
                                self.misses += 1;

                                // SET NOT FULL
                                if set.tags.len() < self.asso_num {
                                        // Set not full, insert new tag into set
                                        // Find first free index in set
                                        let free_index = (0..self.asso_num)
                                                .find(|i| !set.tags.values().any(|&index| index == *i))
                                                .expect("No free index found");

                                        set.tags.insert(tag, free_index);

                                        if self.rp == 1 {
                                                // Add new tag to end of access order for LRU
                                                set.access_order.push(tag);
                                        }
                                        else if self.rp == 2 {
                                                // Initialise frequency for new tag
                                                set.access_frequency.insert(tag, 0);
                                        }
                                
                                // SET FULL
                                } else {
                                        // Set full, need to evict line
                                        // Select victim tag & index based on replacement policy
                                        let (victim_tag, victim_index) = match self.rp {
                                                0 => {
                                                        let idx = set.rr_counter % self.asso_num;
                                                        set.rr_counter += 1;
                                                        let vtag = *set.tags.iter()
                                                                .find(|&(_, &index)| index == idx)
                                                                .map(|(tag, _)| tag)
                                                                .expect("VTag not found for RR");
                                                        (vtag, idx)
                                                },
                                                1 => {
                                                        // least recently used tag at start of vec
                                                        let vtag = set.access_order.remove(0); 
                                                        let idx = *set.tags.get(&vtag)
                                                                .expect("VTag not found for LRU");
                                                        (vtag, idx)
                                                },
                                                2 => {
                                                        // least frequently used tag tiebroken by lowest index
                                                        // tiebreak ensures deterministic behaviour
                                                        let vtag = set.access_frequency.iter()
                                                                .min_by_key(|&(&tag, &freq)| {
                                                                        let index = *set.tags.get(&tag)
                                                                                .expect("LFU tag not found");
                                                                        (freq, index)
                                                                })
                                                                .map(|(&tag, _)| tag)
                                                                .expect("No LFU tags found");
                                                        let idx = *set.tags.get(&vtag)
                                                                .expect("VTag nbot found for LFU");
                                                        (vtag, idx)
                                                },
                                                _ => panic!("Invalid replacement policy"),
                                        };

                                        // Remove victim from all tracking structs
                                        set.tags.remove(&victim_tag);
                                        if self.rp == 1 {
                                                set.access_order.retain(|&t| t != victim_tag);
                                        }
                                        else if self.rp == 2 {
                                                set.access_frequency.remove(&victim_tag);
                                        }

                                        // Insert new tag
                                        set.tags.insert(tag, victim_index);
                                        if self.rp == 1 {
                                                set.access_order.push(tag);
                                        }
                                        else if self.rp == 2 {
                                                set.access_frequency.insert(tag, 0);
                                        }
                                }

                                // On miss, access lower-level cache if exists, otherwise "access main memory"
                                if let Some(lower_cache) = &mut self.lower {
                                        // Fetch entire cache line
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

impl NWayCache {

        // extract set index from address
        fn get_set_index(&self, address: u64) -> usize {
                let mask = (1u64 << self.index_bits) - 1;
                ((address >> self.offset_bits) & mask) as usize
        }

        // extract tag from address
        fn get_tag(&self, address: u64) -> u64 {
                address >> (self.offset_bits + self.index_bits)
        }

}