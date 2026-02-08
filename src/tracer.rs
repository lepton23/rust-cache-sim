
use std::fs::File;
use memmap2::Mmap;
use std::thread;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;


#[derive(Debug, Clone, Copy)]
pub struct TraceEntry {
        pub address: u64,
        pub size: u8,
}

#[derive(Copy, Clone)]
struct SendPtr(*mut TraceEntry);
unsafe impl Send for SendPtr {}
unsafe impl Sync for SendPtr {}


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
unsafe fn parse_decimal_u8(ptr: *const u8) -> u8 {
        unsafe { 
                let d0 = (*ptr.add(0)).wrapping_sub(b'0'.into());
                let d1 = (*ptr.add(1)).wrapping_sub(b'0'.into());
                let d2 = (*ptr.add(2)).wrapping_sub(b'0'.into());
                d0 * 100 + d1 * 10 + d2
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
                size: unsafe { parse_decimal_u8(ptr.add(36)) },
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
                let chunk = unsafe { _mm256_loadu_si256(data.as_ptr().add(offset) as *const __m256i) };
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

        // get number of threads and get chunk size per thread
        let num_threads = thread::available_parallelism()?.get();
        let chunk_size = mmap.len() / num_threads;

        // // touch pages in parallel to warm caches before parsing
        // (0..num_threads).into_iter().for_each(|i| {
        //         let start = i * chunk_size;
        //         let end = ((i + 1) * chunk_size).min(mmap.len());
        //         let mut dummy = 0u8;
        //         for idx in (start..end).step_by(4096) {
        //                 unsafe { dummy ^= mmap.get_unchecked(idx) };
        //         }
        //         std::hint::black_box(dummy);
        // });

        // search for chunk boundaries
        let mut chunk_starts = Vec::with_capacity(num_threads + 1);
        chunk_starts.push(0);
        
        for i in 1..num_threads {
                let pos = i * chunk_size;
                if pos >= mmap.len() {
                        break;
                }
                // Find newline, convert to absolute position, and skip past it
                if let Some(relative_pos) = find_newline_simd(&mmap[pos..]) {
                        chunk_starts.push(pos + relative_pos + 1);  // +1 to start after the newline
                } else {
                        chunk_starts.push(mmap.len());
                        break;
                }
        }
        chunk_starts.push(mmap.len());

        // First pass over mmap: count lines per chunk in parallel
        let chunk_line_counts: Vec<usize> = thread::scope(|s| {
                let handles: Vec<_> = (0..chunk_starts.len() - 1)
                .map(|i| {
                        let start = chunk_starts[i];
                        let end = chunk_starts[i + 1];

                        // copy mmap reference into closure
                        let mmap_ref = &mmap;

                        s.spawn(move || {
                                let chunk = &mmap_ref[start..end];
                                chunk.split(|&b| b == b'\n')
                                        .filter(|line| line.len() == 39)
                                        .count()
                        })
                })
                .collect();
                
                handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        // calculate offsets for each chunk in the final buffer
        let mut offsets = vec![0usize; chunk_line_counts.len() + 1];
        for i in 0..chunk_line_counts.len() {
                offsets[i + 1] = offsets[i] + chunk_line_counts[i];
        }
        let total_lines = offsets[chunk_line_counts.len()];

        // pre-allocate entire output buffer
        let mut entries = Vec::with_capacity(total_lines);
        unsafe { entries.set_len(total_lines); }

        // wrap raw pointer in SendPtr to allow sending across threads safely
        let send_ptr = SendPtr(entries.as_mut_ptr());

        // Second pass: parse directly into pre-allocated positions
        thread::scope(|s| {
                let handles: Vec<_> = (0..chunk_starts.len() - 1)
                        .map(|i| {
                                let start = chunk_starts[i];
                                let end = chunk_starts[i + 1];
                                let offset = offsets[i];
                                let expected_count = chunk_line_counts[i];

                                // copy mmap reference into closure
                                let mmap_ref = &mmap;
                                
                                // Explicitly move send_ptr into a local variable
                                let ptr = &send_ptr;
                                
                                // println!("Spawning thread for chunk {} with offset {} and {} lines", i, offset, expected_count);
                                s.spawn(move || {
                                        let chunk = &mmap_ref[start..end];
                                        let mut local_offset = 0usize;

                                        // Extract the raw pointer from SendPtr
                                        let entries_ptr = ptr.0;
                                        
                                        for line in chunk.split(|&b| b == b'\n') {
                                                if line.len() == 39 {
                                                        // assert!(local_offset < expected_count, 
                                                        //         "Chunk {}: parsed more lines ({}) than counted ({})", 
                                                        //         i, local_offset + 1, expected_count);
                                                        unsafe {
                                                                std::ptr::write(entries_ptr.add(offset + local_offset), parse_trace_line(line));
                                                        }
                                                        local_offset += 1;
                                                }
                                        }
                                        // println!("Thread for chunk {} parsed {} lines", i, local_offset);
                                })
                        })
                        .collect();
                
                for handle in handles {
                        handle.join().unwrap();
                }
        });



        Ok(entries)
}