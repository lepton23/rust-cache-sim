
use std::fs::File;
use memmap2::Mmap;
use rayon::prelude::*;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;


#[derive(Debug, Clone, Copy)]
pub struct TraceEntry {
        pub address: u64,
        pub op_type: u8,
        pub size: u16,
}

// static hex lookup table prepared at compile time
// alighn to cache line size
#[repr(align(64))]
struct HexLut([u8; 256]);

static HEX_LUT: HexLut = HexLut({
        let mut lut = [255u8; 256];
        let mut i = 0;
        while i < 10 {
                lut[(b'0' + i) as usize] = i;
                i += 1;
        }
        i = 0;
        while i < 6 {
                lut[(b'a' + i) as usize] = 10 + i;
                lut[(b'A' + i) as usize] = 10 + i;
                i += 1;
        }
        lut
});

// assume valid input for maximum speed
// use get_unchecked to avoid bounds checks since we assume input is valid
#[inline(always)]
unsafe fn parse_hex_u16(ptr: *const u8) -> u16 {
        let lut = &HEX_LUT.0;
        unsafe { 
                (*lut.get_unchecked(*ptr.add(0) as usize) as u16) << 8
                | (*lut.get_unchecked(*ptr.add(1) as usize) as u16) << 4
                | (*lut.get_unchecked(*ptr.add(2) as usize) as u16)
        }
}

#[inline(always)]
unsafe fn parse_hex_u64(ptr: *const u8) -> u64 {
        let lut = &HEX_LUT.0;
        // load nibbles first as its better for CPU pipelining
        unsafe {
                let n0 = *lut.get_unchecked(*ptr.add(0) as usize) as u64;
                let n1 = *lut.get_unchecked(*ptr.add(1) as usize) as u64;
                let n2 = *lut.get_unchecked(*ptr.add(2) as usize) as u64;
                let n3 = *lut.get_unchecked(*ptr.add(3) as usize) as u64;
                let n4 = *lut.get_unchecked(*ptr.add(4) as usize) as u64;
                let n5 = *lut.get_unchecked(*ptr.add(5) as usize) as u64;
                let n6 = *lut.get_unchecked(*ptr.add(6) as usize) as u64;
                let n7 = *lut.get_unchecked(*ptr.add(7) as usize) as u64;
                let n8 = *lut.get_unchecked(*ptr.add(8) as usize) as u64;
                let n9 = *lut.get_unchecked(*ptr.add(9) as usize) as u64;
                let n10 = *lut.get_unchecked(*ptr.add(10) as usize) as u64;
                let n11 = *lut.get_unchecked(*ptr.add(11) as usize) as u64;
                let n12 = *lut.get_unchecked(*ptr.add(12) as usize) as u64;
                let n13 = *lut.get_unchecked(*ptr.add(13) as usize) as u64;
                let n14 = *lut.get_unchecked(*ptr.add(14) as usize) as u64;
                let n15 = *lut.get_unchecked(*ptr.add(15) as usize) as u64;
                
                (n0 << 60) | (n1 << 56) | (n2 << 52) | (n3 << 48)
                | (n4 << 44) | (n5 << 40) | (n6 << 36) | (n7 << 32)
                | (n8 << 28) | (n9 << 24) | (n10 << 20) | (n11 << 16)
                | (n12 << 12) | (n13 << 8) | (n14 << 4) | n15
        }
}

// read trace file in parallel
#[inline(always)]
unsafe fn parse_trace_line(line: &[u8]) -> TraceEntry {
        let ptr = line.as_ptr();
        TraceEntry {
                address: unsafe { parse_hex_u64(ptr.add(17)) },
                op_type: *ptr.add(34),
                size: unsafe { parse_hex_u16(ptr.add(36)) },
        }
}


// Use SIMD to find newline positions for chunking the file in parallel
#[inline(always)]
fn find_newline_simd(data: &[u8]) -> Option<usize> {
        // lab PCs are x86_64 - using intel i5 14500 chips which support AVX2
        #[cfg(target_arch = "x86_64")]
        {
                if is_x86_feature_detected!("avx2") {
                        unsafe { return find_newline_avx2(data); }
                }
        }
        data.iter().position(|&b| b == b'\n')
}

// AVX2 is an x86 extension that allows processing of 32 bytes in parallel in 256 bit wide registers using SIMD instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn find_newline_avx2(data: &[u8]) -> Option<usize> {

        // we initialise a needle register with 32 copies of the newline character
        // the set1_epi8 function sets all 32 bytes of the register to the given value
        let needle = _mm256_set1_epi8(b'\n' as i8);
        let mut offset = 0;

        // we load 32 bytes of the data into a register and compare it to the needle register using the cmpeq function
        // we then use movemask to get a 32 bit mask where each bit corresponds to if the bytes were equal or not
        // i.e. locating all newline characters in the 32 byte chunk in one instruction
        while offset + 32 <= data.len() {
                let chunk = _mm256_loadu_si256(data.as_ptr().add(offset) as *const __m256i);
                let cmp = _mm256_cmpeq_epi8(chunk, needle);
                let mask = _mm256_movemask_epi8(cmp);

                if mask != 0 {
                        return Some(offset + mask.trailing_zeros() as usize);
                }
                offset += 32;
        }

        // check remaining bytes
        data[offset..].iter().position(|&b| b == b'\n').map(|pos| offset + pos)
}


pub fn read_trace(path: &str) -> Result<Vec<TraceEntry>, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        // advise kernel of access pattern for better readahead
        unsafe {
                libc::madvise(
                        mmap.as_ptr() as *mut libc::c_void,
                        mmap.len(),
                        libc::MADV_WILLNEED | libc::MADV_SEQUENTIAL
                );
        }

        // 40 bytes per line in trace file including newlines
        let estimated_lines = mmap.len() / 40; 

        let num_threads = rayon::current_num_threads();
        let chunk_size = mmap.len() / num_threads;

        // touch pages in parallel to warm caches before parsing
        (0..rayon::current_num_threads()).into_par_iter().for_each(|i| {
                let start = i * chunk_size;
                let end = ((i + 1) * chunk_size).min(mmap.len());
                let mut dummy = 0u8;
                for idx in (start..end).step_by(4096) {
                        dummy ^= mmap[idx];
                }
                std::hint::black_box(dummy);
        });

        // parallel search for chunk boundaries
        let mut chunk_starts: Vec<usize> = (0..num_threads)
                .into_par_iter()
                .map(|i| {
                        if i == 0 {
                                return 0;
                        }
                        let pos = i * chunk_size;
                        if pos >= mmap.len() {
                                return mmap.len();
                        }
                        // Find next newline
                        find_newline_simd(&mmap[pos..])
                                .map(|offset| pos + offset + 1) // +1 to start after newline
                                .unwrap_or(mmap.len()) // if no newline found, end of file
                })
                .collect();

        chunk_starts.push(mmap.len());

        // parse chunks in parallel using the precomputed chunk boundaries, collecting results into a single vector
        let entries: Vec<_> = chunk_starts.par_windows(2)
                .fold(
                        || Vec::with_capacity(estimated_lines / num_threads),
                        |mut acc, window| {
                                let chunk = &mmap[window[0]..window[1]];
                                for line in chunk.split(|&b| b == b'\n') {
                                        if line.len() == 39 {
                                                acc.push(unsafe { parse_trace_line(line) });
                                        }
                                }
                                acc
                        }
                )
                .reduce(
                        || Vec::with_capacity(estimated_lines),
                        |mut acc, chunk_entries| {
                                acc.extend(chunk_entries);
                                acc
                        }
                );

        Ok(entries)
}