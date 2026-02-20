
use crate::caches::cachemeta::CacheMeta;
use crate::Cache;

use rustc_hash::FxHashMap;
use rustc_hash::FxBuildHasher;

#[inline]
#[cold]
fn cold() {}

#[inline]
fn likely(b: bool) -> bool {
    if !b { cold() }
    b
}

pub struct FullAssocCache<L: Cache> {
        offset_bits: u32,
        num_lines: usize,
        line_size: u64,
        occupied: usize,

        // Cache storage
        
        // Store hashmap of tags to their index in the cache
        tags: Vec<u64>,
        tags_map: FxHashMap<u64, usize>,

        // For replacement policies track order & frequency
        // use flat arrays for cache locality
        global_clock: u64,
        rr_counter: usize, // for RR
        lru_timestamps: Vec<u64>, // for LRU
        lfu_frequencies: Vec<u64>, // for LFU

        hits: u64,
        misses: u64,

        // Store a reference to lower level cache if exists
        lower: L,
        rp: u8, // replacement policy - 0 = RR, 1 = LRU, 2 = LFU
}

const INVALID_TAG: u64 = u64::MAX;

impl<L: Cache> FullAssocCache<L> {
        pub fn new(meta: &CacheMeta, lower: L) -> Self {
                // Calculate no of cache lines
                let num_lines = (meta.size / meta.line_size) as usize;

                // Calculate bits for offset, index, and tag
                let offset_bits = meta.line_size.trailing_zeros();

                // Initialise cache
                FullAssocCache {
                        offset_bits,
                        num_lines,
                        line_size: meta.line_size,
                        occupied: 0,
                        tags: vec![INVALID_TAG; num_lines],
                        tags_map: FxHashMap::with_capacity_and_hasher(num_lines, FxBuildHasher),
                        global_clock: 0,
                        rr_counter: 0,
                        lru_timestamps: vec![0; num_lines],
                        lfu_frequencies: vec![0; num_lines],
                        hits: 0,
                        misses: 0,
                        lower,
                        rp: meta.rp,
                }
        }

       
        #[inline(always)]
        fn on_hit(&mut self, idx: usize) {
                match self.rp {
                        0 => {},
                        1 => {
                                // LRU update access order
                                self.global_clock += 1;
                                unsafe { *self.lru_timestamps.get_unchecked_mut(idx) = self.global_clock };
                        },
                        2 => {
                                // LFU update access frequency
                                unsafe { *self.lfu_frequencies.get_unchecked_mut(idx) += 1 };
                        },
                        _ => unreachable!(),
                }
        }

        #[inline(always)]
        fn find_victim(&mut self) -> usize {
                // if cache not full, use next empty slot
                if self.occupied < self.num_lines {
                        let idx = self.occupied;
                        self.occupied += 1;
                        return idx;
                }

                match self.rp {
                        0 => {
                                // RR
                                let idx = self.rr_counter % self.num_lines;
                                self.rr_counter += 1;
                                idx
                        },
                        1 => {
                                // LRU: find min timestamp
                                let mut min_ts = u64::MAX;
                                let mut min_idx = 0;
                                for i in 0..self.num_lines {
                                        let ts = unsafe { *self.lru_timestamps.get_unchecked(i) };
                                        if ts < min_ts {
                                                min_ts = ts;
                                                min_idx = i;
                                        }
                                }
                                min_idx
                        },
                        2 => {
                                // LFU: find min freqeuncy, tiebreak w/ index
                                let mut min_freq = u64::MAX;
                                let mut min_idx = 0;
                                for i in 0..self.num_lines {
                                        let freq = unsafe { *self.lfu_frequencies.get_unchecked(i) };
                                        if freq < min_freq {
                                                min_freq = freq;
                                                min_idx = i;
                                        }
                                }
                                min_idx
                        },
                        _ => unreachable!(),
                }
        }

        #[inline(always)]
        fn replace_line(&mut self, idx: usize, tag: u64) {
                // on eviction, remove old tag from map
                let old_tag = self.tags[idx];
                if old_tag != INVALID_TAG {
                        self.tags_map.remove(&old_tag);
                }
                self.tags_map.insert(tag, idx);
                
                unsafe { *self.tags.get_unchecked_mut(idx) = tag };

                match self.rp {
                        0 => {},
                        1 => {
                                // LRU update access order
                                self.global_clock += 1;
                                unsafe { *self.lru_timestamps.get_unchecked_mut(idx) = self.global_clock };
                        },
                        2 => {
                                // LFU update access frequency
                                unsafe { *self.lfu_frequencies.get_unchecked_mut(idx) = 0 };
                        },
                        _ => unreachable!(),
                }
        } 
}
        
impl<L: Cache> Cache for FullAssocCache<L> {
        // "Perform access"
        fn access(&mut self, address: u64, size: u64) {
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
                let tag = line_index;
                
                let found = self.tags_map.get(&tag).copied();

                if let Some(idx) = found {
                        // HIT
                        self.hits += 1;
                        self.on_hit(idx);
                } else {
                        // MISS
                        self.misses += 1;
                        let victim_idx = self.find_victim();
                        self.replace_line(victim_idx, tag);

                        self.lower.access(line_index << self.offset_bits, self.line_size);
                }
        }

        // Recursively get metadata on hits and misses from this cache and all lower caches
        // Format into vec
        fn get_hits(&self, mut hits_vec: Vec<u64>) -> Vec<u64> {

                hits_vec.push(self.hits);
                hits_vec = self.lower.get_hits(hits_vec);

                hits_vec
        }

        fn get_misses(&self, mut misses_vec: Vec<u64>) -> Vec<u64> {

                misses_vec.push(self.misses);
                misses_vec = self.lower.get_misses(misses_vec);
                
                misses_vec
        }

        // Get main mem accesses from lowest cache
        fn get_main_memory_accesses(&self) -> u64 {
                self.lower.get_main_memory_accesses()
        }
}

