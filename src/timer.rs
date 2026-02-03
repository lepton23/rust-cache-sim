
use std::time::{Instant, Duration};

pub struct Timer {
        start: Instant,
        label: String,
}

impl Timer {
        pub fn new(label: &str) -> Self {
                println!("Starting: {}", label);
                Timer {
                        start: Instant::now(),
                        label: label.to_string(),
                }
        }

        pub fn elapsed(&self) -> Duration {
                self.start.elapsed()
        }

        pub fn stop(&self) -> Duration {
                let duration = self.start.elapsed();
                println!("Finished: {} in {:?}", self.label, duration);
                duration
        }
}

pub struct ProfileData {
        pub trace_size_bytes: u64,
        pub trace_entries: usize,
        pub read_duration: Duration,
}

impl ProfileData {
        pub fn print_summary(&self) {

                println!("\n=============================");
                println!("=== Trace File Statistics ===");
                println!("File size: {} bytes ({:.2} MB)", 
                        self.trace_size_bytes, 
                        self.trace_size_bytes as f64 / 1_048_576.0
                );
                println!("Entries loaded: {}", self.trace_entries);
                println!("Read time: {:?}", self.read_duration);
                println!("Throughput: {:.2} MB/s", 
                        (self.trace_size_bytes as f64 / 1_048_576.0) / self.read_duration.as_secs_f64()
                );
                println!("Entries/sec: {:.2} million", 
                        self.trace_entries as f64 / self.read_duration.as_secs_f64() / 1_000_000.0
                );
                println!("=============================\n");
        }

}