
use crate::caches::cachemeta::CacheMeta;
use crate::Cache;
use crate::caches::cache::AnyCache;

#[inline]
#[cold]
fn cold() {}

#[inline]
fn likely(b: bool) -> bool {
    if !b { cold() }
    b
}

#[inline]
fn unlikely(b: bool) -> bool {
    if b { cold() }
    b
}


struct CacheSet {
        tags: [u64; 8], // tag to index in cache lines
        rr_counter: usize,
        access_order: [u64; 8],
        access_order_len: u8, // how many valid tags for LRU
        access_frequency: [u64; 8],
}

const INVALID_TAG: u64 = u64::MAX;

impl CacheSet {
        fn new() -> Self {
                CacheSet {
                        tags: [INVALID_TAG; 8],
                        rr_counter: 0,
                        access_order: [0; 8],
                        access_order_len: 0,
                        access_frequency: [0; 8],
                }
        }

        // Remove first tag occurence from access_order then shift others left
        fn order_remove(&mut self, tag: u64) {
                let len = self.access_order_len as usize;
                if let Some(pos) = self.access_order[..len].iter().position(|&t| t == tag) {
                        // shift the rest left
                        for i in pos..len - 1 {
                                self.access_order[i] = self.access_order[i + 1];
                        }
                        self.access_order_len -= 1;
                }
        }

        // Push tag to end of access_order
        fn order_push(&mut self, tag: u64) {
                let len = self.access_order_len as usize;
                self.access_order[len] = tag;
                self.access_order_len += 1;
        }

        // Remove and return the oldest tag
        fn order_pop_front(&mut self) -> u64 {
                let tag = self.access_order[0];
                let len = self.access_order_len as usize;
                for i in 0..len - 1 {
                        self.access_order[i] = self.access_order[i + 1];
                }
                self.access_order_len -= 1;
                tag
        }

        // Move tag to back as most recently sued
        fn order_move_to_back(&mut self, tag: u64) {
                self.order_remove(tag);
                self.order_push(tag);
        }
}

pub struct NWayCache {
        offset_bits: u32,
        index_bits: u32,
        index_mask: u64,
        line_size: u64,
        asso_num: usize,

        // Cache storage //
        // Store vec of sets rather than vec of tags
        // for replacement policies track order & frequency within each set
        sets: Vec<CacheSet>,        

        hits: u64,
        misses: u64,
        main_memory_accesses: u64,

        // Store a reference to lower level cache if exists
        lower: Option<Box<AnyCache>>,

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

                let index_mask = (1u64 << index_bits) - 1;

                // Initialise sets
                let sets = (0..num_sets)
                        .map(|_| CacheSet::new())
                        .collect();

                // Construct cache
                NWayCache {
                        offset_bits,
                        index_bits,
                        index_mask,
                        line_size: meta.line_size,
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
        fn access(&mut self, address: u64, size: u64) -> () {
                // Calculate which cache lines are accessed (& check for accesses that span across lines)
                // end_line has -1 since if we access the last byte of a line, it should still count as accessing that line
                let start_line = address >> self.offset_bits;
                let end_line = (address + size - 1) >> self.offset_bits; 

                if likely(start_line == end_line) {
                        self.access_line(start_line);
                } else {
                        // Access each cache line that the address + size spans
                        for line_index in start_line..=end_line {
                                self.access_line(line_index);
                        }
                }

        }
       
        #[inline(always)]
        fn access_line(&mut self, line_index: u64) {
                // Calculate which cache lines are accessed (& check for accesses that span across lines)
                // end_line has -1 since if we access the last byte of a line, it should still count as accessing that line
                let line_address = line_index << self.offset_bits;
                let set_index = self.get_set_index(line_address);
                let tag = self.get_tag(line_address);


                let set = &mut self.sets[set_index];

                // Find tag in set
                let hit_index = set.tags.iter()
                        .position(|&t| t == tag);

                // Check for hit
                if let Some(index) = hit_index {
                        // HIT
                        self.hits += 1;

                        if self.rp == 1 {
                                // Update access order for LRU
                                set.order_move_to_back(tag);
                        }
                        if self.rp == 2 {
                                // Update access frequency for LFU
                                set.access_frequency[index] += 1;
                        }
                } else {
                        // MISS
                        self.misses += 1;

                        // Find empty index
                        let empty_index = set.tags[..self.asso_num].iter()
                                .position(|&t| t == INVALID_TAG);

                        // SET NOT FULL
                        if let Some(free_index) = empty_index {
                                // Set not full, insert new tag into set
                                // Find first free index in set
                                set.tags[free_index] = tag;

                                if self.rp == 1 {
                                        // Add new tag to end of access order for LRU
                                        set.order_push(tag);
                                }
                                else if self.rp == 2 {
                                        // Initialise frequency for new tag
                                        set.access_frequency[free_index] = 1;
                                }
                        
                        // SET FULL
                        } else {
                                // Set full, need to evict line
                                // Select victim tag & index based on replacement policy
                                let victim_index = match self.rp {
                                        0 => {
                                                let idx = set.rr_counter % self.asso_num;
                                                set.rr_counter = set.rr_counter.wrapping_add(1);
                                                idx
                                        },
                                        1 => {
                                                // least recently used tag at start of vec
                                                let vtag = set.order_pop_front();
                                                let idx = set.tags[..self.asso_num].iter()
                                                        .position(|&t| t == vtag)
                                                        .expect("Victim not found for LRU");
                                                idx
                                        },
                                        2 => {
                                                // least frequently used tag tiebroken by lowest index
                                                // tiebreak ensures deterministic behaviour
                                                let idx = set.access_frequency[..self.asso_num].iter()
                                                        .enumerate()
                                                        .filter(|(i, _)| set.tags[*i] != INVALID_TAG)
                                                        .min_by_key(|&(ref i, &freq)| (freq, *i))
                                                        .map(|(i, _)| i)
                                                        .expect("No LFU victim found");
                                                idx
                                        },
                                        _ => panic!("Invalid replacement policy"),
                                };

                                let victim_tag = set.tags[victim_index];

                                // Remove victim from all tracking structs
                                if self.rp == 1 {
                                        set.order_remove(victim_tag);
                                }

                                // Insert new tag
                                set.tags[victim_index] = tag;
                                if self.rp == 1 {
                                        set.order_push(tag);
                                }
                                else if self.rp == 2 {
                                        set.access_frequency[victim_index] = 1;
                                }
                        }

                        // On miss, access lower-level cache if exists, otherwise "access main memory"
                        if let Some(lower_cache) = &mut self.lower {
                                // Fetch entire cache line
                                lower_cache.access(line_address, self.line_size);
                        } else {
                                self.main_memory_accesses += 1;
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

impl NWayCache {

        // extract set index from address
        #[inline(always)]
        fn get_set_index(&self, address: u64) -> usize {
                ((address >> self.offset_bits) & self.index_mask) as usize
        }

        // extract tag from address
        #[inline(always)]
        fn get_tag(&self, address: u64) -> u64 {
                address >> (self.offset_bits + self.index_bits)
        }

}