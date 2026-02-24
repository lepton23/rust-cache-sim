
// #[global_allocator]
// static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

// originally tried using mimalloc as an allocator alternative but no noticeable performance difference

use std::env;
use std::io;
use std::io::Write;

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


// Macro to dispatch on cache type and create appropriate cache struct, then run body on that struct
// It's a bit ugly, but allows us to avoid dynamic dispatch and still have a clean way to build the cache hierarchy
// - that doesn't infinitely recurse
macro_rules! dispatch {
        ($meta:expr, $lower:expr, $body:expr) => {
                match $meta.kind {
                        0 => { let cache = DirectCache::new(&$meta.clone(), $lower); $body(cache) }
                        1 => { let cache = NWayCache::<2, _>::new(&$meta.clone(), $lower); $body(cache) }
                        2 => { let cache = NWayCache::<4, _>::new(&$meta.clone(), $lower); $body(cache) }
                        3 => { let cache = NWayCache::<8, _>::new(&$meta.clone(), $lower); $body(cache) }
                        4 => { let cache = FullAssocCache::new(&$meta.clone(), $lower); $body(cache) }
                        _ => panic!("Unsupported cache type"),
                }
        };
}



fn main() {
        let args: Vec<String> = env::args().collect();
        let json_path = &args[1];
        let trace_path = &args[2];

        // Read json file
        let cachemetas: Vec<CacheMeta> = read_json(json_path);

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
                0 => panic!("Please sir, just one cache..."),
                1 => {
                        let _ = dispatch!(&cachemetas[0], mem, |mut top| run(&mut top, tp, cn));
                }
                2 => {
                        let _ = dispatch!(&cachemetas[1], mem, |l2| {
                                        dispatch!(&cachemetas[0], l2, |mut top| run(&mut top, tp, cn))
                        });
                }
                3 => {
                        let _ = dispatch!(&cachemetas[2], mem, |l3| {
                                        dispatch!(&cachemetas[1], l3, |l2| {
                                                dispatch!(&cachemetas[0], l2, |mut top| run(&mut top, tp, cn))
                                        })
                        });
                }
                _ => {
                        // for more than 3 cache levels
                        // levels are built dynamically then top 3 are build monomorphically for best performance
                        let dyn_bottom: Box<dyn Cache> = Box::new(mem);
                        let dyn_metas: Vec<CacheMeta> = cachemetas[3..].iter().rev().cloned().collect();
                        let dyn_chain = build_dyn_chain(&dyn_metas, dyn_bottom);

                        let _ = dispatch!(&cachemetas[2], dyn_chain, |l3| {
                                        dispatch!(&cachemetas[1], l3, |l2| {
                                                dispatch!(&cachemetas[0], l2, |mut top| run(&mut top, tp, cn))
                                        })
                        });
                }
        }
} // main


fn build_dyn_chain(metas: &[CacheMeta], bottom: Box<dyn Cache>) -> Box<dyn Cache> {
        let mut current = bottom;
        for meta in metas {
                current = dispatch_boxed(meta, current);
        }
        current
}

fn dispatch_boxed(meta: &CacheMeta, lower: Box<dyn Cache>) -> Box<dyn Cache> {
        match meta.kind {
                0 => Box::new(DirectCache::new(meta, lower)),
                1 => Box::new(NWayCache::<2, _>::new(meta, lower)),
                2 => Box::new(NWayCache::<4, _>::new(meta, lower)),
                3 => Box::new(NWayCache::<8, _>::new(meta, lower)),
                4 => Box::new(FullAssocCache::new(meta, lower)),
                _ => panic!("Unsupported cache type"),
        }
}

// Run tracefile entries through cache hierarchy
fn run<C: Cache>(
        cache: &mut C,
        trace_path: &str,
        cache_names: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
        // start processing tracefile & run through cache sim
        run_trace(cache, trace_path);
                
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


        print!("{json_output}");
        io::stdout().flush().unwrap();

        let mut output_file = std::fs::File::create("results.json")?;
        std::io::Write::write_all(&mut output_file, json_output.as_bytes())?;
        
        Ok(())

} // run


fn run_trace<C: Cache>(top_cache: &mut C, trace_path: &str) {
        let _ = process_trace(trace_path, |entry| {
                top_cache.access(entry.address, entry.size);
        });
}


