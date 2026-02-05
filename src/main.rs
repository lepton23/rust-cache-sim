
mod timer;
use timer::{Timer, ProfileData};

mod tracer;
use tracer::read_trace;

/*
CS4202 - Cache Simulation
*/



fn main() -> Result<(), Box<dyn std::error::Error>> {
        let trace_path = "trace-files/bwaves.out";

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

