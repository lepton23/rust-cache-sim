
use std::fs::File;
use memmap2::Mmap;
use rayon::prelude::*;

mod timer;
use timer::{Timer, ProfileData};

mod tracer;
use tracer::read_trace;

/*
CS4202 - Cache Simulation
*/


// prefetch tracefile into memory 
fn warm_caches(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        // advise kernel ahead that we need file sequentially
        unsafe {
                libc::madvise(
                        mmap.as_ptr() as *mut libc::c_void,
                        mmap.len(),
                        libc::MADV_WILLNEED | libc::MADV_SEQUENTIAL
                );
        }

        // touch pages in parallel to warm caches
        let chunk_size = mmap.len() / rayon::current_num_threads();
        (0..rayon::current_num_threads()).into_par_iter().for_each(|i| {
                let start = i * chunk_size;
                let end = ((i + 1) * chunk_size).min(mmap.len());
                let mut dummy = 0u8;
                for idx in (start..end).step_by(4096) {
                        dummy ^= mmap[idx];
                }
                std::hint::black_box(dummy);
        });

        Ok(())
}


fn main() -> Result<(), Box<dyn std::error::Error>> {
        let trace_path = "trace-files/bwaves.out";

        // Warm up caches
        warm_caches(trace_path)?;

        let file_size = std::fs::metadata(trace_path)?.len();

        // Time read operation
        let timer = Timer::new("Reading trace file");
        let entries = read_trace(trace_path)?;
        let read_duration = timer.stop();

        // Profile data
        let profile = ProfileData {
                trace_size_bytes: file_size,
                trace_entries: entries.len(),
                read_duration,
        };

        profile.print_summary();

        if !entries.is_empty() {
                println!("First entry: addr={:016x}, op={} hex={:02x}, size={:03x}", 
                        entries[0].address, 
                        entries[0].op_type as char,
                        entries[0].op_type,
                        entries[0].size
                );
                println!("9th entry: addr={:016x}, op={} hex={:02x}, size={:03x}", 
                        entries[8].address, 
                        entries[8].op_type as char,
                        entries[8].op_type,
                        entries[8].size
                );
        }

        Ok(())
}

