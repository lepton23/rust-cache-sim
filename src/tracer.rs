
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
static HEX_LUT: [u8; 256] = {
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
};

// assume valid input for maximum speed
#[inline(always)]
fn parse_hex_u64(bytes: &[u8]) -> u64 {
        ((HEX_LUT[bytes[0] as usize] as u64) << 60)
        | ((HEX_LUT[bytes[1] as usize] as u64) << 56)
        | ((HEX_LUT[bytes[2] as usize] as u64) << 52)
        | ((HEX_LUT[bytes[3] as usize] as u64) << 48)
        | ((HEX_LUT[bytes[4] as usize] as u64) << 44)
        | ((HEX_LUT[bytes[5] as usize] as u64) << 40)
        | ((HEX_LUT[bytes[6] as usize] as u64) << 36)
        | ((HEX_LUT[bytes[7] as usize] as u64) << 32)
        | ((HEX_LUT[bytes[8] as usize] as u64) << 28)
        | ((HEX_LUT[bytes[9] as usize] as u64) << 24)
        | ((HEX_LUT[bytes[10] as usize] as u64) << 20)
        | ((HEX_LUT[bytes[11] as usize] as u64) << 16)
        | ((HEX_LUT[bytes[12] as usize] as u64) << 12)
        | ((HEX_LUT[bytes[13] as usize] as u64) << 8)
        | ((HEX_LUT[bytes[14] as usize] as u64) << 4)
        | (HEX_LUT[bytes[15] as usize] as u64)
}

#[inline(always)]
fn parse_hex_u16(bytes: &[u8]) -> u16 {
        ((HEX_LUT[bytes[0] as usize] as u16) << 8)
        | ((HEX_LUT[bytes[1] as usize] as u16) << 4)
        | (HEX_LUT[bytes[2] as usize] as u16)
}

// Parse single tracefile line
#[inline(always)]
unsafe fn parse_trace_line(line: &[u8]) -> TraceEntry {

        TraceEntry {
                address: parse_hex_u64(&line[17..33]),
                op_type: *line.get_unchecked(34),
                size: parse_hex_u16(&line[36..39]),
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

        let num_threads = rayon::current_num_threads();
        let chunk_size = mmap.len() / num_threads;

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

        // parse chunks in parallel - using flat_map_iter to avoid intermediate vecs
        let entries: Vec<_> = chunk_starts.par_windows(2)
                .flat_map_iter(|window| {
                        let chunk = &mmap[window[0]..window[1]];
                        let estimated_lines = chunk.len() / 40;
                        let mut chunk_entries = Vec::with_capacity(estimated_lines);

                        for line in chunk.split(|&b| b == b'\n') {
                                if line.len() == 39 {
                                        chunk_entries.push(unsafe { parse_trace_line(line) });
                                }
                        }
                        chunk_entries 
                })
                .collect();

        Ok(entries)
}