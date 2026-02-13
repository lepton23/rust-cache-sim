
use std::env;

use serde::{Serialize, Deserialize};

mod tracer;
use tracer::process_trace;

mod json_reader;
use json_reader::read_json;

mod caches;
use caches::{
        CacheMeta, cache::Cache, 
        cache::AnyCache,
};

/*
CS4202 - Cache Simulation
*/


#[derive(Serialize, Deserialize)]
struct CacheResult {
        hits: u64,
        misses: u64,
        name: String,
}

#[derive(Serialize, Deserialize)]
struct SimulationResults {
        caches: Vec<CacheResult>,
        main_memory_accesses: u64,
}


fn main() -> Result<(), Box<dyn std::error::Error>> {

        let args: Vec<String> = env::args().collect();

        let json_path = &args[1];
        let trace_path = &args[2];


        // Read json file
        let cachemetas: Vec<CacheMeta> = read_json(json_path)?;

        // Store cache names in order (top to bottom)
        let cache_names: Vec<String> = cachemetas.iter()
                .map(|meta| meta.name.clone())
                .collect();

        // Initialise caches from bottom up (lowest level first)
        let mut caches: Vec<Box<AnyCache>> = Vec::with_capacity(cachemetas.len());

        for meta in cachemetas.into_iter().rev() {
                let mut cache: Box<AnyCache> = Box::new(AnyCache::new(meta));

                // if we have a lower level cache, connect it
                if let Some(lower) = caches.pop() {
                        cache.set_lower(lower);
                }

                caches.push(cache);
        }

        let mut top_cache = caches.pop().expect("Need at least one cache");

        // Stream trace entries in batches directly into cache structure
        process_trace(trace_path, |batch| {
                for entry in batch {
                        let entry = unsafe { entry.assume_init() };
                        top_cache.access(entry.address, entry.size);
                }
        })?;

        // Get total hits and misses for all caches
        let hits = top_cache.get_hits(Vec::new());
        let misses = top_cache.get_misses(Vec::new());
        let mma = top_cache.get_main_memory_accesses();

        // Build results structure
        let cache_results: Vec<CacheResult> = cache_names.iter()
                .enumerate()
                .map(|(i, name)| CacheResult {
                        hits: hits[i],
                        misses: misses[i],
                        name: name.clone(),
                })
                .collect();

        let results = SimulationResults {
                caches: cache_results,
                main_memory_accesses: mma,
        };

        let json_output = serde_json::to_string_pretty(&results)?;
        println!("{}", json_output);
        let mut output_file = std::fs::File::create("results.json")?;
        std::io::Write::write_all(&mut output_file, json_output.as_bytes())?;

        Ok(())
}

