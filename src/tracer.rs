
use std::fs::File;
use memmap2::Mmap;
use std::thread;

// #[cfg(target_arch = "x86_64")]
// use std::arch::x86_64::*;


#[derive(Debug, Clone, Copy)]
pub struct TraceEntry {
        pub address: u64,
        pub size: u64,
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
unsafe fn parse_decimal_u64(ptr: *const u8) -> u64 {
        unsafe { 
                let d0 = (*ptr.add(0) - b'0') as u64;
                let d1 = (*ptr.add(1) - b'0') as u64;
                let d2 = (*ptr.add(2) - b'0') as u64;

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
                size: unsafe { parse_decimal_u64(ptr.add(36)) },
        }
}


// // Use SIMD to find newline positions for chunking the file in parallel
// #[inline(always)]
// fn find_newline_simd(data: &[u8]) -> Option<usize> {
//         // lab PCs are x86_64 - using intel i5 14500 chips which support AVX2
//         #[cfg(target_arch = "x86_64")]
//         {
//                 if is_x86_feature_detected!("avx2") {
//                         unsafe { return find_newline_avx2(data); }
//                 }
//         }
//         data.iter().position(|&b| b == b'\n')
// }

// // AVX2 is an x86 extension that allows processing of 32 bytes in parallel in 256 bit wide registers using SIMD instructions.
// #[cfg(target_arch = "x86_64")]
// #[target_feature(enable = "avx2")]
// unsafe fn find_newline_avx2(data: &[u8]) -> Option<usize> {

//         // we initialise a needle register with 32 copies of the newline character
//         // the set1_epi8 function sets all 32 bytes of the register to the given value
//         let needle = _mm256_set1_epi8(b'\n' as i8);
//         let mut offset = 0;

//         // we load 32 bytes of the data into a register and compare it to the needle register using the cmpeq function
//         // we then use movemask to get a 32 bit mask where each bit corresponds to if the bytes were equal or not
//         // i.e. locating all newline characters in the 32 byte chunk in one instruction
//         while offset + 32 <= data.len() {
//                 let chunk = unsafe { _mm256_loadu_si256(data.as_ptr().add(offset) as *const __m256i) };
//                 let cmp = _mm256_cmpeq_epi8(chunk, needle);
//                 let mask = _mm256_movemask_epi8(cmp);

//                 if mask != 0 {
//                         return Some(offset + mask.trailing_zeros() as usize);
//                 }
//                 offset += 32;
//         }

//         // check remaining bytes
//         data[offset..].iter().position(|&b| b == b'\n').map(|pos| offset + pos)
// }

const BATCH_SIZE: usize = 1 << 16;

pub fn process_trace<F>(path: &str, mut process: F) -> Result<(), Box<dyn std::error::Error>>
where
        F: FnMut(&[TraceEntry]),
{
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
        let line_width = 40; // 40 bytes per line in trace file (39 chars + newline)
        let total_lines = mmap.len() / line_width;

        let mut batch = vec![TraceEntry { address: 0, size: 0 }; BATCH_SIZE];
        let mut offset = 0;


        while offset < total_lines{
                let batch_end = (offset + BATCH_SIZE).min(total_lines);
                let batch_len = batch_end - offset;
                let chunk_size = (batch_len + num_threads - 1) / num_threads; // ceiling division

                // wrap raw pointer in SendPtr to allow sending across threads safely
                let send_ptr = SendPtr(batch.as_mut_ptr());

                thread::scope(|s| {
                        for t in 0..num_threads {
                                let start = t * chunk_size;
                                let end = ((t + 1) * chunk_size).min(total_lines);
                                
                                // guard to avoid spawning empty threads
                                if start >= end {
                                        continue; // no more lines to process
                                }

                                // copy mmap reference into closure
                                let mmap_ref = &mmap;
                                
                                // Explicitly move send_ptr into a local variable
                                let ptr = &send_ptr;
                                
                                s.spawn(move || {
                                        // Extract the raw pointer from SendPtr
                                        let entries_ptr = ptr.0;
                                        
                                        for i in start..end {
                                                let line_offset = (offset + i) * line_width;
                                                let line = unsafe { 
                                                        mmap_ref.get_unchecked(line_offset..line_offset + line_width) 
                                                };
                                                unsafe {
                                                        std::ptr::write(entries_ptr.add(i), parse_trace_line(line));
                                                }
                                        }
                                });
                        }
                });

                process(&batch[..batch_len]);
                offset = batch_end;
        }

        Ok(())
}