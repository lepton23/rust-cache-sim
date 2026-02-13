
use std::fs::File;
use memmap2::MmapOptions;
use std::mem::MaybeUninit;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;


#[derive(Debug, Clone, Copy)]
pub struct TraceEntry {
        pub address: u64,
        pub size: u64,
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
unsafe fn parse_decimal_u64(ptr: *const u8) -> u64 {
        unsafe { 
                let d0 = (*ptr.add(0) - b'0') as u64;
                let d1 = (*ptr.add(1) - b'0') as u64;
                let d2 = (*ptr.add(2) - b'0') as u64;

                d0 * 100 + d1 * 10 + d2
        }
}

#[inline(always)]
unsafe fn parse_hex_u64_no_simd(ptr: *const u8) -> u64 {
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

// Use SIMD to parse lines in one go!
// AVX2 is an x86 extension that allows processing of 32 bytes in parallel in 256 bit wide registers using SIMD instructions.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
#[inline]
unsafe fn parse_line_pair_simd(ptr0: *const u8, ptr1: *const u8) -> (u64, u64) {
        // load 16 hex chars into 128 bit register for each line
        let lo = unsafe { _mm_loadu_si128(ptr0 as *const __m128i) };
        let hi = unsafe { _mm_loadu_si128(ptr1 as *const __m128i) };
        let input = _mm256_set_m128i(hi, lo); // combine into 256 bit register

        // extract low nibble of each byte
        let mask_0f = _mm256_set1_epi8(0x0f); // this is an 8 bit needle which we can AND over our 256 bit input
        let low_nibbles = _mm256_and_si256(input, mask_0f); // as so - grabbing the low nibble of each byte

        // digits: 0-9: 0x30 -> 0x39
        // letters: a-f: 0x61 -> 0x66, A-F: 0x41 -> 0x46
        // so if byte > 0x39, its a letter and needs +9 offset        
        let mask_39 = _mm256_set1_epi8(0x39u8 as i8);
        let is_letter = _mm256_cmpgt_epi8(input, mask_39);

        // add 9 to letter nibbles to convert from 0-5 to 10-15
        let nine_offset = _mm256_set1_epi8(9);
        let letter_offset = _mm256_and_si256(is_letter, nine_offset);

        // get final nibble values
        let nibbles = _mm256_add_epi8(low_nibbles, letter_offset);

        let evens_shifted = _mm256_slli_epi16::<4>(nibbles);
        let evens_mask = _mm256_set1_epi16(0x00f0u16 as i16);
        let evens = _mm256_and_si256(evens_shifted, evens_mask);

        let odds_mask = _mm256_set1_epi16(0xff00u16 as i16);
        let odds = _mm256_and_si256(nibbles, odds_mask);
        let odds_shifted = _mm256_srli_epi16::<8>(odds);

        // shift low bytes into position as so
        let combined = _mm256_or_si256(evens, odds_shifted);

        // now we have 8 bytes in even byte positions of each 16 bit word in 16 byte address
        // use pshufb to pack them back together
        let shuffle = _mm256_set_epi8(
                -1, -1, -1, -1, -1, -1, -1, -1,
                14, 12, 10, 8, 6, 4, 2, 0,
                -1, -1, -1, -1, -1, -1, -1, -1,
                14, 12, 10, 8, 6, 4, 2, 0 
        );
        let packed = _mm256_shuffle_epi8(combined, shuffle);

        // extract each 128-bit lane and pull out low 64 bits
        let lane0 = _mm256_castsi256_si128(packed);
        let lane1 = _mm256_extracti128_si256::<1>(packed);

        let result0 = (_mm_cvtsi128_si64(lane0) as u64).swap_bytes(); 
        let result1 = (_mm_cvtsi128_si64(lane1) as u64).swap_bytes();
        
        // return both results
        (result0, result1)

}


const BATCH_SIZE: usize = 1 << 17;

pub fn process_trace<F>(path: &str, mut process: F) -> Result<(), Box<dyn std::error::Error>>
where
        F: FnMut(&[MaybeUninit<TraceEntry>]),
{

        let file = File::open(path)?;

        // prefault all pages to avoid page faults during processing
        let mmap = unsafe { MmapOptions::new().populate().map(&file)? };

        // advise kernel of access pattern for better readahead
        unsafe {
                libc::madvise(
                        mmap.as_ptr() as *mut libc::c_void,
                        mmap.len(),
                        libc::MADV_WILLNEED | libc::MADV_SEQUENTIAL
                );
        }

        let line_width = 40; // 40 bytes per line in trace file (39 chars + newline)
        let total_lines = mmap.len() / line_width;

        let mut batch: Vec<MaybeUninit<TraceEntry>> = Vec::with_capacity(BATCH_SIZE);
        unsafe { batch.set_len(BATCH_SIZE) }; // we will initialize in place, so set length to capacity

        let mut offset = 0;
        while offset < total_lines{
                let batch_end = (offset + BATCH_SIZE).min(total_lines);
                let batch_len = batch_end - offset;
                                        
                // process paris of lines with dual-lane AVX2
                let pairs = batch_len / 2;
                for i in 0..pairs {
                        let idx = i * 2;
                        let line_offset0 = (offset + idx) * line_width;
                        let line_offset1 = (offset + idx + 1) * line_width;

                        // prefetch 8 lines ahead
                        if idx + 8 < batch_len {
                                unsafe {
                                        // _MM_HINT_T0 means to prefetch into all levels of cache
                                        let future_offset0 = (offset + idx + 8) * line_width;
                                        let future_offset1 = (offset + idx + 9) * line_width;
                                        _mm_prefetch(mmap.as_ptr().add(future_offset0) as *const i8, _MM_HINT_T0);
                                        _mm_prefetch(mmap.as_ptr().add(future_offset1) as *const i8, _MM_HINT_T0);
                                }
                        }
                        
                        unsafe {
                                let ptr0 = mmap.as_ptr().add(line_offset0);
                                let ptr1 = mmap.as_ptr().add(line_offset1);

                                let (addr0, addr1) = parse_line_pair_simd(ptr0.add(17), ptr1.add(17));

                                // debug_assert_eq!(addr0, parse_hex_u64_no_simd(ptr0.add(17)), 
                                //         "SIMD mismatch at line {}, simd={:x}, no_simd={:x}", offset + idx, addr0, parse_hex_u64_no_simd(ptr0.add(17)));
                                // debug_assert_eq!(addr1, parse_hex_u64_no_simd(ptr1.add(17)),
                                //         "SIMD mismatch at line {}, simd={:x}, no_simd={:x}", offset + idx + 1, addr1, parse_hex_u64_no_simd(ptr1.add(17)));

                                batch[idx] = MaybeUninit::new(TraceEntry {
                                        address: addr0,
                                        size: parse_decimal_u64(ptr0.add(36)),
                                });
                                batch[idx + 1] = MaybeUninit::new(TraceEntry {
                                        address: addr1,
                                        size: parse_decimal_u64(ptr1.add(36)),
                                });
                        }
                }
                
                // process last line if batch_len is odd
                if batch_len % 2 != 0 {
                        let last = batch_len - 1;
                        let line_offset = (offset + last) * line_width;
                        unsafe {
                                let ptr = mmap.as_ptr().add(line_offset);
                                batch[last] = MaybeUninit::new(TraceEntry {
                                        address: parse_hex_u64_no_simd(ptr.add(17)),
                                        size: parse_decimal_u64(ptr.add(36)),
                                });
                        }
                }       
                        
                process(&batch[..batch_len]);
                offset = batch_end;
        }

        Ok(())
}