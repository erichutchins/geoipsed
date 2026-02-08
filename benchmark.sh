#!/bin/bash
#
# Benchmark script comparing geoipsed performance using Hyperfine
# Compares installed version with the local build
#

set -e

# Define colors for output
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Paths
INSTALLED_GEOIPSED="/Users/hutch/.cargo/bin/geoipsed"
NEW_GEOIPSED="./target/release/geoipsed"

# Check for hyperfine
if ! command -v hyperfine &> /dev/null; then
    echo -e "${RED}Error: hyperfine is not installed${NC}"
    echo -e "Please install it with: cargo install hyperfine"
    echo -e "Or visit: https://github.com/sharkdp/hyperfine"
    exit 1
fi

# Build the release version
echo -e "${BLUE}Building release version of geoipsed...${NC}"
cargo build --release

# Check if installed version exists
if [ ! -f "$INSTALLED_GEOIPSED" ]; then
    echo -e "${YELLOW}Warning: Installed version not found at $INSTALLED_GEOIPSED${NC}"
    echo -e "${YELLOW}Using new version for both benchmarks${NC}"
    INSTALLED_GEOIPSED="$NEW_GEOIPSED"
fi

# Get versions
INSTALLED_VERSION=$($INSTALLED_GEOIPSED --version)
NEW_VERSION=$($NEW_GEOIPSED --version)

echo -e "${BLUE}Comparing:${NC}"
echo -e "  Installed: ${YELLOW}$INSTALLED_VERSION${NC}"
echo -e "  New:       ${YELLOW}$NEW_VERSION${NC}"
echo

# Create a test file with many IP addresses if it doesn't exist
TEST_FILE="/tmp/geoipsed_benchmark.txt"

if [ ! -f "$TEST_FILE" ]; then
    echo -e "${BLUE}Creating test file with IP addresses...${NC}"
    {
        for i in {1..1000}; do
            echo "Server log entry with IPv4 address 93.184.216.$((i % 255)) and IPv6 address 2001:db8::$i"
        done
    } > "$TEST_FILE"
fi

# Function to run a benchmark
run_benchmark() {
    local name=$1
    local cmd=$2
    local cmd_installed=${cmd/NEW_GEOIPSED/$INSTALLED_GEOIPSED}
    local cmd_new=${cmd/NEW_GEOIPSED/$NEW_GEOIPSED}

    echo -e "\n${BLUE}Benchmark: ${YELLOW}$name${NC}"
    echo -e "${BLUE}Command: ${NC}${cmd//NEW_GEOIPSED/geoipsed}"

    # Use hyperfine for accurate benchmarking
    hyperfine --warmup 2 \
              --min-runs 5 \
              --export-markdown "/tmp/hyperfine_${name// /_}.md" \
              --export-json "/tmp/hyperfine_${name// /_}.json" \
              --prepare "sleep 0.5" \
              --command-name "Installed" "$cmd_installed > /dev/null" \
              --command-name "New" "$cmd_new > /dev/null"

    # Print the improvement factor from the JSON file
    if [ -f "/tmp/hyperfine_${name// /_}.json" ]; then
        old_time=$(jq '.results[0].mean' "/tmp/hyperfine_${name// /_}.json")
        new_time=$(jq '.results[1].mean' "/tmp/hyperfine_${name// /_}.json")
        if (( $(echo "$old_time > 0" | bc -l) )); then
            improvement=$(echo "scale=2; ($old_time / $new_time)" | bc)
            echo -e "${GREEN}Improvement factor: ${improvement}x${NC}"
        fi
    fi
}

# Function to run a benchmark for new features
run_new_feature_benchmark() {
    local name=$1
    local cmd=$2
    local cmd_new=${cmd/NEW_GEOIPSED/$NEW_GEOIPSED}

    echo -e "\n${BLUE}Benchmark: ${YELLOW}$name${NC} (New feature only)"
    echo -e "${BLUE}Command: ${NC}${cmd//NEW_GEOIPSED/geoipsed}"

    # Use hyperfine for new feature benchmark
    hyperfine --warmup 2 \
              --min-runs 5 \
              --export-markdown "/tmp/hyperfine_${name// /_}.md" \
              --prepare "sleep 0.5" \
              --command-name "New" "$cmd_new > /dev/null"
}

# Run various benchmarks

# Basic IP decoration
run_benchmark "Basic IP decoration" \
    "cat $TEST_FILE | NEW_GEOIPSED"

# Only matching mode
run_benchmark "Only matching mode" \
    "cat $TEST_FILE | NEW_GEOIPSED -o"

# Custom template
run_benchmark "Custom template" \
    "cat $TEST_FILE | NEW_GEOIPSED -t \"{ip} in {country_iso}\""

# No private IPs
run_new_feature_benchmark "Exclude private IPs" \
    "cat $TEST_FILE | NEW_GEOIPSED --no-private"

# All IPs (new feature)
run_new_feature_benchmark "Include all IPs" \
    "cat $TEST_FILE | NEW_GEOIPSED --all"

# Tag mode (JSON output) - only for new version
run_new_feature_benchmark "Tag mode (JSON output)" \
    "cat $TEST_FILE | $NEW_GEOIPSED --tag"

echo -e "\n${BLUE}Benchmark complete! 🎉${NC}"
echo -e "Detailed results are saved in /tmp/hyperfine_*.{md,json}"
