use std::env;

use serde::{Serialize, Deserialize};

mod tracer;
use tracer::process_trace;

mod json_reader;
use json_reader::read_json;

mod caches;
use caches::{
        CacheMeta, cache::Cache, 
        cache::MainMemory,
        direct::DirectCache,
        n_way::NWayCache,
        fullassoc::FullAssocCache,
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

macro_rules! dispatch {
        ($meta:expr, $lower:expr, $body:expr) => {
                match $meta.kind {
                        0 => { let cache = DirectCache::new($meta.clone(), $lower); $body(cache) }
                        1 | 2 | 3 => { let cache = NWayCache::new($meta.clone(), $lower); $body(cache) }
                        4 => { let cache = FullAssocCache::new($meta.clone(), $lower); $body(cache) }
                        _ => panic!("Unsupported cache type"),
                }
        };
}

fn run<C: Cache>(
        cache: &mut C,
        trace_path: &str,
        cache_names: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
        run_trace(cache, trace_path)?;
                
        // Get total hits and misses for all caches
        let hits = cache.get_hits(Vec::new());
        let misses = cache.get_misses(Vec::new());
        let mma = cache.get_main_memory_accesses();

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


fn run_trace<C: Cache>(top_cache: &mut C, trace_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        process_trace(trace_path, |entry| {
                top_cache.access(entry.address, entry.size);
        })
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

        // build cache hierarchy and get top cache
        // then run trace through recursive hierarchy
        // Streaming trace entries in batches directly into cache structure
        let mem = MainMemory::new();
        let tp = trace_path.as_str();
        let cn = &cache_names;

        match cachemetas.len() {
                1 => {
                        dispatch!(&cachemetas[0], mem, |mut top| run(&mut top, tp, cn));
                }
                2 => {
                        dispatch!(&cachemetas[1], mem, |l2| {
                                dispatch!(&cachemetas[0], l2, |mut top| run(&mut top, tp, cn))
                        });
                }
                3 => {
                        dispatch!(&cachemetas[2], mem, |l3| {
                                dispatch!(&cachemetas[1], l3, |l2| {
                                        dispatch!(&cachemetas[0], l2, |mut top| run(&mut top, tp, cn))
                                })
                        });
                }
                _ => panic!("Max 3 cache levels supported"),
        }

        Ok(())
}

