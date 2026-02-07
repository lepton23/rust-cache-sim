
use std::env;

mod timer;
use timer::{Timer, ProfileData};

mod tracer;
use tracer::read_trace;

mod json_reader;
use json_reader::read_json;

mod caches;
use caches::CacheMeta;

/*
CS4202 - Cache Simulation
*/

fn main() -> Result<(), Box<dyn std::error::Error>> {

        let args: Vec<String> = env::args().collect();

        let json_path = &args[1];
        let trace_path = &args[2];

        let json_path = "sample-inputs/l1l2l3.json";
        let trace_path = "trace-files-ly/bwaves.out";

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


        Ok(())
}

