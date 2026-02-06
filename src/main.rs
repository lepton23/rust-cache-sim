
use std::env;
use std::fs;

use serde_json::{Value};

mod timer;
use timer::{Timer, ProfileData};

mod tracer;
use tracer::read_trace;

/*
CS4202 - Cache Simulation
*/


// use same data type for each field of struct for cache alignment
struct Cache {
        level:          u64,
        size:           u64,
        line_size:      u64,
        kind:           u8, // 0 = direct, 1 = 2way, 2 = 4way, 3 = 8way, 4 = fullassoc
        rp:             u8, // 0 = rr, 1 = lru, 2 = lfu
}




fn main() -> Result<(), Box<dyn std::error::Error>> {

        let args: Vec<String> = env::args().collect();

        let json_path = &args[1];
        let trace_path = &args[2];

        let json_path = "sample-inputs/direct.json";
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


        let json_data = fs::read_to_string(json_path)?;

        // read in json
        let json: serde_json::Value = 
                serde_json::from_str(&json_data).expect("\nJSON formatted incorrectly.\n\n");
        
        println!("{:?}", json);


        Ok(())
}

