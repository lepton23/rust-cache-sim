import subprocess
import time
import json
import sys
import os

def profile_cache_sim(cache_file, trace_file, language="rust"):
        """
        Profile the cache simulator and output timing data to JSON.
        
        Args:
                cache_file: Path to the cache configuration JSON file
                trace_file: Path to the trace file
                language: Programming language used (default: "Rust")
        
        Returns:
                dict: Profile data including timing and configuration info
        """
        
        # Get base names for output
        cache_name = os.path.basename(cache_file)
        trace_name = os.path.basename(trace_file)
        
        # Build the release binary first
        print("Building release binary...")
        build_result = subprocess.run(
                ["cargo", "build", "--release"],
                capture_output=True,
                text=True
        )
        
        if build_result.returncode != 0:
                print(f"Build failed:\n{build_result.stderr}")
                return None
        
        print("Build successful!")
        
        # Run the binary and time it
        binary_path = "./target/release/p1-cachesim"  # Adjust to your binary name
        
        print(f"Running simulation: {cache_name} on {trace_name}")
        
        start_time = time.time()
        
        result = subprocess.run(
                [binary_path, cache_file, trace_file],
                capture_output=True,
                text=True
        )
        
        end_time = time.time()
        elapsed_millis = int((end_time - start_time) * 1000)
        
        if result.returncode != 0:
                print(f"Simulation failed:\n{result.stderr}")
                return None
        
        # Print the output from the simulator
        print("\n--- Simulator Output ---")
        print(result.stdout)
        print("--- End Output ---\n")
        
        # Create profile data
        profile_data = {
                "cache_file": cache_name,
                "trace_file": trace_name,
                "millis": elapsed_millis,
                "language": language
        }
        
        return profile_data


def main():
        if len(sys.argv) < 3:
                print("Usage: python profile.py <cache_config.json> <trace_file>")
                print("Example: python profile.py configs/l1.json traces/gcc.trace")
                sys.exit(1)
        
        cache_file = sys.argv[1]
        trace_file = sys.argv[2]
        
        # Check if files exist
        if not os.path.exists(cache_file):
                print(f"Error: Cache file '{cache_file}' not found")
                sys.exit(1)
        
        if not os.path.exists(trace_file):
                print(f"Error: Trace file '{trace_file}' not found")
                sys.exit(1)
        
        # Run profiling
        profile_data = profile_cache_sim(cache_file, trace_file)
        
        if profile_data is None:
                print("Profiling failed!")
                sys.exit(1)
        
        # Output to JSON file
        output_file = "profile_results.json"
        with open(output_file, 'w') as f:
                json.dump(profile_data, f, indent=2)
        
        print(f"\nProfile Results:")
        print(json.dumps(profile_data, indent=2))
        print(f"\nResults written to {output_file}")
        
        return profile_data


if __name__ == "__main__":
        main()