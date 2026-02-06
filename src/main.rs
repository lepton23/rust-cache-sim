
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
        level:          u8, // 0 = l1, 1 = l2, 2 = l3
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


        // read in json
        let json_data = fs::read_to_string(json_path)?;
        let json: Value = 
                serde_json::from_str(&json_data).expect("\nJSON formatted incorrectly.\n\n");
        
        println!("{:?}", json);

        let json_caches = json["caches"].as_array().expect("json formatting issue");

        let mut caches: Vec::<Cache> = Vec::with_capacity(json_caches.len());

        for cache in json_caches {

                let name = cache["name"].as_str().expect("Expected str for Name");
                let size = cache["size"].as_u64().expect("Expected u64 for size");
                let line_size = cache["line_size"].as_u64().expect("Expected u64 for line size");
                let kind = cache["kind"].as_str().expect("Expected str for kind");

                let rp = "rr";
                if !cache["replacement_policy"].is_null() {
                        let rp = cache["replacement_policy"].as_str().expect("Expected str for replacement policy");
                }

                let name_u8: u8 = match name {
                        "L1" => 0,
                        "L2" => 1,
                        "L3" => 2, 
                        _ => panic!("Unknown cache name: {}", name),
                };

                let size_u64: u64 = size;
                let line_size_u64: u64 = line_size;

                let kind_u8: u8 = match kind {
                        "direct"        => 0,
                        "2way"          => 1,
                        "4way"          => 2,
                        "8way"          => 3,
                        "full"          => 4,
                        _               => panic!("Unkown cache kind: {}", kind),
                };

                let rp_u8: u8 = match rp {
                        "rr"    => 0,
                        "lru"   => 1,
                        "lfu"   => 2,
                        _       => 0,
                };

                caches.push(
                        Cache {
                                level: name_u8, size: size_u64, 
                                line_size: line_size_u64, 
                                kind: kind_u8, rp: rp_u8
                        }
                );
        }


        Ok(())
}

