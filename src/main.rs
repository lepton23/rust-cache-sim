
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

        // let json_path = &args[1];
        // let trace_path = &args[2];

        let json_path = "sample-inputs/l1l2l3direct.json";
        let trace_path = "trace-files-ly/leela.out";

        let file_size = std::fs::metadata(trace_path)?.len();

        // Time read operation
        let trace_timer = Timer::new("Running trace file");

        // Read trace file
        let entries = read_trace(trace_path)?;

        let trace_duration = trace_timer.stop();
        let trace_profile = ProfileData {
                trace_size_bytes: file_size,
                trace_entries: entries.len(),
                read_duration: trace_duration,
        };
        trace_profile.print_summary("Trace File");


        let json_timer = Timer::new("Reading cache metadata from JSON");
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

        json_timer.stop();
        println!("Caches initialised");
        

        let cache_timer = Timer::new("Running trace file on caches");
        // run tracefile on caches
        for entry in &entries {
                top_cache.access(entry.address, entry.size);
        }
        let cache_duration = cache_timer.stop();

        // get total hits and misses for all caches
        let hits = top_cache.get_hits(Vec::new());
        let misses = top_cache.get_misses(Vec::new());
        let mma = top_cache.get_main_memory_accesses();
        let overflow = top_cache.get_overflow_accesses();

        println!("Total hits: {:?}", hits);
        println!("Total misses: {:?}", misses);
        println!("Main memory accesses: {}", mma);
        println!("Overflow accesses (span multiple lines): {}", overflow);


        // Profile data
        let cache_profile = ProfileData {
                trace_size_bytes: file_size,
                trace_entries: entries.len(),
                read_duration: cache_duration,
        };
        cache_profile.print_summary("Cache Simulation");
        
        Ok(())
}

