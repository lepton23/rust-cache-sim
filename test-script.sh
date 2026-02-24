#!/bin/bash

# ============================================================
# Cache Simulator Test Script
# Runs all sample inputs and compares against expected outputs
# ============================================================

BINARY="./target/release/p1-cachesim"
INPUTS_DIR="./sample-inputs"
OUTPUTS_DIR="./sample-outputs"
TRACES_DIR="./trace-files"
ACTUAL_DIR="./actual"

# colours
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # no colour

mkdir -p "$ACTUAL_DIR"

if [ ! -x "$BINARY" ]; then
  echo -e "${RED}Error: binary $BINARY not found or not executable${NC}"
  exit 1
fi

pass=0
fail=0
total=0

# iterate over every expected output file
for expected in "$OUTPUTS_DIR"/output-*.json; do
  filename=$(basename "$expected")

  # extract trace name and config name from filename
  # format: output-{trace}-{config}.json
  inner="${filename#output-}" # strip "output-"
  inner="${inner%.json}"      # strip ".json"

  # split on first '-' to get trace, rest is config
  trace="${inner%%-*}"
  config="${inner#*-}"

  trace_file="$TRACES_DIR/${trace}.trace"
  config_file="$INPUTS_DIR/${config}.json"
  actual_file="$ACTUAL_DIR/${filename}"

  total=$((total + 1))

  # check files exist
  if [ ! -f "$trace_file" ]; then
    echo -e "${YELLOW}SKIP${NC} $filename — trace not found: $trace_file"
    continue
  fi

  if [ ! -f "$config_file" ]; then
    echo -e "${YELLOW}SKIP${NC} $filename — config not found: $config_file"
    continue
  fi

  # run simulator, capture stdout only (stderr has timing)
  "$BINARY" "$config_file" "$trace_file" >"$actual_file" 2>/dev/null

  if [ $? -ne 0 ]; then
    echo -e "${RED}FAIL${NC} $filename — simulator exited with error"
    fail=$((fail + 1))
    continue
  fi

  # compare actual vs expected
  if diff -q <(sed -e '$a\' "$expected") <(sed -e '$a\' "$actual_file") >/dev/null 2>&1; then
    echo -e "${GREEN}PASS${NC} $filename"
    pass=$((pass + 1))
  else
    echo -e "${RED}FAIL${NC} $filename"
    echo "  --- expected"
    echo "  +++ actual"
    diff <(sed -e '$a\' "$expected") <(sed -e '$a\' "$actual_file") | sed 's/^/  /'
    fail=$((fail + 1))
  fi
done

echo ""
echo "========================================"
echo -e "Results: ${GREEN}$pass passed${NC} / ${RED}$fail failed${NC} / $total total"
echo "========================================"

if [ $fail -eq 0 ]; then
  exit 0
else
  exit 1
fi
