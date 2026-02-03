# StormDL

Next-generation download accelerator with adaptive multi-segment parallel downloads.

## Features

- **Adaptive Segmentation**: Automatically calculates optimal segment count based on bandwidth-delay product
- **Protocol Support**: HTTP/1.1, HTTP/2, HTTP/3 (QUIC)
- **Multi-Source Downloads**: Download from multiple mirrors simultaneously
- **Resume Support**: Crash recovery with integrity verification
- **Terminal UI**: Per-segment progress visualization

## Installation

### Quick Install (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/augani/stormdl/main/scripts/install.sh | bash
```

### Package Managers

#### Homebrew (macOS/Linux)
```bash
brew tap augani/stormdl
brew install stormdl
```

#### Arch Linux (AUR)
```bash
yay -S stormdl
# or
paru -S stormdl
```

#### Alpine Linux
```bash
apk add stormdl --repository=https://dl-cdn.alpinelinux.org/alpine/edge/testing
```

#### Debian/Ubuntu
```bash
# Download the .deb from releases
sudo dpkg -i stormdl_0.1.0_amd64.deb
```

#### Fedora/RHEL
```bash
# Download the .rpm from releases
sudo rpm -i stormdl-0.1.0-1.x86_64.rpm
```

### From Source

```bash
# Requires Rust 1.70+
cargo install --git https://github.com/augani/stormdl

# Or clone and build
git clone https://github.com/augani/stormdl
cd stormdl
cargo build --release
```

## Usage

```bash
# Basic download
storm https://example.com/file.zip

# Specify output directory
storm https://example.com/file.zip -o ~/Downloads

# Use 16 segments
storm https://example.com/file.zip -s 16

# Conservative mode for sensitive servers
storm https://example.com/file.zip --gentle

# Multi-source download with mirrors
storm https://mirror1.example.com/file.iso \
  -m https://mirror2.example.com/file.iso \
  -m https://mirror3.example.com/file.iso

# Verify checksum after download
storm https://example.com/file.zip --checksum abc123...
```

## Configuration

Default config location: `~/.config/storm-dl/config.toml`

```toml
[segments]
max_segments = 32
min_segment_size = 262144  # 256 KB

[connections]
per_host_limit = 6
timeout_secs = 300

[io]
write_buffer_size = 1048576  # 1 MB
```

## Performance

### Benchmarks

StormDL significantly outperforms traditional download tools by using parallel segmented downloads:

| File Size | wget | curl | storm (4 seg) | storm (8 seg) | storm (16 seg) | vs wget |
|-----------|------|------|---------------|---------------|----------------|---------|
| 10 MB | 0.4 MB/s | 0.5 MB/s | 1.4 MB/s | 2.5 MB/s | **3.4 MB/s** | **+525%** |
| 100 MB | 0.4 MB/s | 0.4 MB/s | 1.7 MB/s | 3.1 MB/s | **5.6 MB/s** | **+675%** |

*Tested on macOS ARM64, speedtest.tele2.net. Results vary based on network conditions.*

### How It Works

1. **BDP-Based Segmentation**: Calculates optimal parallel connections using bandwidth-delay product
2. **Adaptive Rebalancing**: Splits slow segments to faster connections every 500ms
3. **HTTP/2 Multiplexing**: Reduces connection overhead on modern servers
4. **Write Coalescing**: Batches disk writes to reduce syscall overhead

### Why Faster Than wget/curl?

Traditional tools like wget and curl download files using a single TCP connection. This means:
- They can't fully utilize available bandwidth on high-latency connections
- A single slow server response blocks the entire download
- No parallelism to work around TCP congestion control limitations

StormDL opens multiple connections and downloads different parts of the file simultaneously, then reassembles them. This saturates your available bandwidth much more effectively.

## License

MIT
