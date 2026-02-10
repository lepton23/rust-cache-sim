
# Build Instructions

In the root directory of this project (i.e. parent to `src`, `target`, etc.), run the following command(s):  

`RUSTFLAGS="-C target-cpu=native" cargo build --release`  

To run the cache simulator, run the following command(s):

`./target/release/p1-cachesim <json_path> <trace_path>`