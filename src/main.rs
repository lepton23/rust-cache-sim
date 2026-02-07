
use std::env;

mod timer;
use timer::{Timer, ProfileData};

mod tracer;
use tracer::read_trace;

mod json_reader;
use json_reader::read_json;

mod caches;
use caches::{CacheMeta, cache::Cache, direct::DirectCache};

/*
CS4202 - Cache Simulation
*/

fn main() -> Result<(), Box<dyn std::error::Error>> {

        let args: Vec<String> = env::args().collect();

        let json_path = &args[1];
        let trace_path = &args[2];

        let json_path = "sample-inputs/direct.json";
        let trace_path = "trace-files-ly/bwaves.out";

        let file_size = std::fs::metadata(trace_path)?.len();

        // Time read operation
        let timer = Timer::new("Running trace file");

        // Read trace file
        let entries = read_trace(trace_path)?;

        if !entries.is_empty() {
                println!("First entry: addr={:016x}, size={:03x}", 
                        entries[0].address, 
                        entries[0].size
                );
                println!("9th entry: addr={:016x}, size={:03x}", 
                        entries[8].address, 
                        entries[8].size
                );
        }


        let cachemetas: Vec<CacheMeta> = read_json(json_path)?;

        println!("Cache metadata: {:?}", cachemetas);

        // Initialise caches from bottom up (lowest level first)
        let mut caches: Vec<Box<dyn Cache>> = Vec::with_capacity(cachemetas.len());

        for meta in cachemetas.into_iter().rev() {
                let mut cache = Box::new(DirectCache::new(meta));

                // if we have a lower level cache, connect it
                if let Some(lower) = caches.pop() {
                        cache.set_lower(lower);
                }

                caches.push(cache);
        }

        let mut top_cache = caches.pop().expect("Need at least one cache");

        // run tracefile on caches
        for entry in &entries {
                top_cache.access(entry.address, entry.size);
        }


        // get total hits and misses for all caches
        let mut hits = 0;
        for (i, cache) in caches.iter().enumerate() {
                let meta = cache.get_meta();
                hits += cache.get_hits();
        }

        println!("Total hits: {}", hits);
        println!("Total misses: {}", entries.len() as u64 - hits);


        let read_duration = timer.stop();
        // Profile data
        let profile = ProfileData {
                trace_size_bytes: file_size,
                trace_entries: entries.len(),
                read_duration,
        };

        profile.print_summary();
        
        Ok(())
}

