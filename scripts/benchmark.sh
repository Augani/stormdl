#!/bin/bash
set -euo pipefail

# StormDL Benchmark Script

STORM="./target/release/storm"
OUTPUT_DIR="/tmp/stormdl-benchmark"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1" >&2; }
success() { echo -e "${GREEN}[OK]${NC} $1" >&2; }

cleanup() {
    rm -rf "$OUTPUT_DIR"
}

benchmark_wget() {
    local url=$1
    local output=$2
    wget -q --spider "$url" 2>/dev/null || true
    local start=$(python3 -c 'import time; print(time.time())')
    wget -q -O "$output" "$url" 2>/dev/null
    local end=$(python3 -c 'import time; print(time.time())')
    python3 -c "print(f'{$end - $start:.2f}')"
}

benchmark_curl() {
    local url=$1
    local output=$2
    curl -s --head "$url" >/dev/null 2>&1 || true
    local start=$(python3 -c 'import time; print(time.time())')
    curl -s -o "$output" "$url"
    local end=$(python3 -c 'import time; print(time.time())')
    python3 -c "print(f'{$end - $start:.2f}')"
}

benchmark_storm() {
    local url=$1
    local output_dir=$2
    local segments=${3:-8}
    local start=$(python3 -c 'import time; print(time.time())')
    $STORM "$url" -o "$output_dir" -s "$segments" --quiet 2>/dev/null
    local end=$(python3 -c 'import time; print(time.time())')
    python3 -c "print(f'{$end - $start:.2f}')"
}

run_benchmark() {
    local name=$1
    local url=$2
    local size_mb=$3

    info "Testing $name ($size_mb MB)..."
    mkdir -p "$OUTPUT_DIR"

    info "  wget..."
    local wget_time=$(benchmark_wget "$url" "$OUTPUT_DIR/wget-$name")
    local wget_speed=$(python3 -c "print(f'{$size_mb / $wget_time:.1f}')")
    success "  wget: ${wget_time}s (${wget_speed} MB/s)"

    info "  curl..."
    local curl_time=$(benchmark_curl "$url" "$OUTPUT_DIR/curl-$name")
    local curl_speed=$(python3 -c "print(f'{$size_mb / $curl_time:.1f}')")
    success "  curl: ${curl_time}s (${curl_speed} MB/s)"

    info "  storm (4 segments)..."
    rm -f "$OUTPUT_DIR"/*.zip 2>/dev/null || true
    local storm4_time=$(benchmark_storm "$url" "$OUTPUT_DIR" 4)
    local storm4_speed=$(python3 -c "print(f'{$size_mb / $storm4_time:.1f}')")
    success "  storm (4): ${storm4_time}s (${storm4_speed} MB/s)"

    info "  storm (8 segments)..."
    rm -f "$OUTPUT_DIR"/*.zip 2>/dev/null || true
    local storm8_time=$(benchmark_storm "$url" "$OUTPUT_DIR" 8)
    local storm8_speed=$(python3 -c "print(f'{$size_mb / $storm8_time:.1f}')")
    success "  storm (8): ${storm8_time}s (${storm8_speed} MB/s)"

    info "  storm (16 segments)..."
    rm -f "$OUTPUT_DIR"/*.zip 2>/dev/null || true
    local storm16_time=$(benchmark_storm "$url" "$OUTPUT_DIR" 16)
    local storm16_speed=$(python3 -c "print(f'{$size_mb / $storm16_time:.1f}')")
    success "  storm (16): ${storm16_time}s (${storm16_speed} MB/s)"

    local improvement=$(python3 -c "print(f'{($storm8_speed / $wget_speed - 1) * 100:.0f}' if $wget_speed > 0 else 'N/A')")

    echo "| $name | ${wget_speed} MB/s | ${curl_speed} MB/s | ${storm4_speed} MB/s | ${storm8_speed} MB/s | ${storm16_speed} MB/s | +${improvement}% |"

    rm -rf "$OUTPUT_DIR"
}

main() {
    echo "# StormDL Performance Benchmark"
    echo ""
    echo "Testing on: $(uname -s) $(uname -m)"
    echo "Date: $(date -u +'%Y-%m-%d %H:%M UTC')"
    echo ""

    if [ ! -f "$STORM" ]; then
        echo "Error: Storm binary not found. Run 'cargo build --release' first." >&2
        exit 1
    fi

    echo "## Results"
    echo ""
    echo "| File Size | wget | curl | storm (4) | storm (8) | storm (16) | vs wget |"
    echo "|-----------|------|------|-----------|-----------|------------|---------|"

    run_benchmark "10MB" "http://speedtest.tele2.net/10MB.zip" 10
    run_benchmark "100MB" "http://speedtest.tele2.net/100MB.zip" 100

    echo ""
    echo "## Notes"
    echo "- All tests performed sequentially with DNS warm-up"
    echo "- 'vs wget' compares storm (8 segments) vs wget"
    echo "- Results vary based on network conditions and server capabilities"
}

trap cleanup EXIT
main "$@"
