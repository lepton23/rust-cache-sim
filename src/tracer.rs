
use std::fs::File;
use memmap2::Mmap;
use rayon::prelude::*;


#[derive(Debug, Clone, Copy)]
pub struct TraceEntry {
        pub address: u64,
        pub op_type: u8,
        pub size: u16,
}

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

// Read in trace file as fast as possible
// Time how long it takes

pub fn read_trace(path: &str) -> Result<Vec<TraceEntry>, Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        let chunk_size = mmap.len() / rayon::current_num_threads().max(1);
        let mut chunk_starts = vec![0];

        for i in 1..rayon::current_num_threads() {
                let pos = (i * chunk_size).min(mmap.len());

                // find next newline
                if let Some(newline_pos) = mmap[pos..].iter().position(|&b| b == b'\n') {
                        chunk_starts.push(pos + newline_pos + 1);
                } 
        }
        chunk_starts.push(mmap.len());

        let entries: Vec<_> = chunk_starts.par_windows(2)
                .flat_map(|window| {
                        let chunk = &mmap[window[0]..window[1]];
                                chunk.split(|&b| b == b'\n')
                                        .filter(|line| line.len() == 39)
                                        .map(|line| unsafe {parse_trace_line(line)})
                                        .collect::<Vec<_>>()
                })
                .collect();

        Ok(entries)
}