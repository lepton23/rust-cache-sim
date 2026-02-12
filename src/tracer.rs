
use std::fs::File;
use memmap2::Mmap;

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
unsafe fn parse_hex_u64_simd(ptr: *const u8) -> u64 {
        // load 16 hex chars into 128 bit register
        let input = _mm_loadu_si128(ptr as *const __m128i);

        // digits: value = byte & 0x0f
        // letters: value = (byte & 0x0f) + 9
        // 'a' & 0x0f = 1 'A' & 0x0f = 1, so we can just grab the low nibble and add 9
        let mask_0f = _mm_set1_epi8(0x0f); // this is an 8 bit needle which we can run over our 128 bit input
        let low_nibbles = _mm_and_si128(input, mask_0f); // as so - grabbing the low nibble of each byte

        // mask to check for letteres
        // we can check if byte & 0x40 != 0
        let mask_f0 = _mm_set1_epi8(0x40);
        let is_letter = _mm_and_si128(input, mask_f0);

        // Convert the 0x40 flag into a 9 offset: shift right by 6, multiply by 9
        // is_letter bytes are 0x40 where letter and 0x00 for digit
        // shift right by 6 to get 0x01 for letter and 0x00 for digit
        let letter_shift = _mm_srli_epi64::<6>(is_letter);
        // multiply by 9: (x << 3) + x
        let letter_x8 = _mm_slli_epi64::<3>(letter_shift);
        let letter_offset = _mm_add_epi8(letter_x8, letter_shift);

        // get final nibble values
        let nibbles = _mm_add_epi8(low_nibbles, letter_offset);

        // now we have 16 4bit values in 16 bytes: [n0, n1, n2, etc]
        // pack the nibble into bytes as so (n0<<4) | n1, (n2<<4) | n3, etc
        let high_nibbles = _mm_slli_epi16::<4>(nibbles); // shift positions left 4
        let high_mask = _mm_set1_epi16(0x00f0u16 as i16); // mask to keep only high nibbles after shift
        let low_mask = _mm_set1_epi16(0x000fu16 as i16); // mask to keep only low nibbles
        let high = _mm_and_si128(high_nibbles, high_mask);
        let low = _mm_and_si128(nibbles, low_mask);

        // shift low bytes into position as so
        let combined = _mm_or_si128(high, _mm_srli_epi16::<8>(low));

        // now we have 8 bytes in even byte positions of each 16 bit word in 16 byte address
        // use pshufb to pack them back together
        let shuffle = _mm_set_epi8(
                -1, -1, -1, -1, -1, -1, -1, -1,
                14, 12, 10, 8, 6, 4, 2, 0
        );
        let packed = _mm_shuffle_epi8(combined, shuffle);

        // extract the final 64 bit value from cvtsi128 as u64
        let result = _mm_cvtsi128_si64(packed) as u64;
        
        // result was big endian so swap bytes
        result.swap_bytes()

}

#[inline(always)]
unsafe fn parse_hex_u64(ptr: *const u8) -> u64 {
        #[cfg(target_arch = "x86_64")]
        {
                if is_x86_feature_detected!("avx2") {
                        return unsafe { parse_hex_u64_simd(ptr) };
                }
        }
        unsafe { parse_hex_u64_no_simd(ptr) }
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



const BATCH_SIZE: usize = 1 << 15;

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

        let line_width = 40; // 40 bytes per line in trace file (39 chars + newline)
        let total_lines = mmap.len() / line_width;

        let mut batch = vec![TraceEntry { address: 0, size: 0 }; BATCH_SIZE];
        let mut offset = 0;


        while offset < total_lines{
                let batch_end = (offset + BATCH_SIZE).min(total_lines);
                let batch_len = batch_end - offset;
                                        
                        for i in 0..batch_len {
                                let line_offset = (offset + i) * line_width;
                                let line = unsafe { 
                                        mmap.get_unchecked(line_offset..line_offset + line_width) 
                                };
                                batch[i] = unsafe { parse_trace_line(line) }; 
                        }

                process(&batch[..batch_len]);
                offset = batch_end;
        }

        Ok(())
}