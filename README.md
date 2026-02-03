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
curl -fsSL https://raw.githubusercontent.com/augustusotu/stormdl/main/scripts/install.sh | bash
```

### Package Managers

#### Homebrew (macOS/Linux)
```bash
brew tap augustusotu/stormdl
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
cargo install --git https://github.com/augustusotu/stormdl

# Or clone and build
git clone https://github.com/augustusotu/stormdl
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

StormDL uses several techniques to maximize download speed:

1. **BDP-Based Segmentation**: Calculates optimal parallel connections using bandwidth-delay product
2. **Adaptive Rebalancing**: Splits slow segments to faster connections every 500ms
3. **HTTP/2 Multiplexing**: Reduces connection overhead on modern servers
4. **Write Coalescing**: Batches disk writes to reduce syscall overhead

## License

MIT
