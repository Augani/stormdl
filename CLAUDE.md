# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

StormDL is a next-generation download accelerator written in Rust. It uses adaptive multi-segment parallel downloads to saturate available bandwidth, supporting HTTP/1.1, HTTP/2, HTTP/3 (QUIC), and FTP protocols.

## Build Commands

```bash
cargo build                          # Build CLI (default)
cargo build --release                # Release build
cargo build --features gui           # Build with GUI
cargo test                           # Run all tests
cargo test -p stormdl-core           # Test specific crate
cargo run -- <URL>                   # Download a file
cargo run -- <URL> -s 8              # Download with 8 segments
cargo run --features gui             # Run GUI (when no URL provided)
cargo clippy                         # Lint
cargo fmt                            # Format
```

## Architecture

The system is organized into horizontal layers with data flowing downward (URL → orchestrator → segments → protocol → I/O → disk) and events flowing upward.

### Crate Structure

| Crate | Purpose |
|-------|---------|
| `stormdl-core` | Zero-dep types and traits: `ByteRange`, `ResourceInfo`, `DownloadState`, `Downloader` trait, `DataSink` trait |
| `stormdl-segment` | Segment manager with adaptive splitting. `SegmentManager` handles split/merge, `Rebalancer` splits slow segments |
| `stormdl-protocol` | HTTP client via reqwest. `HttpDownloader` implements `Downloader` trait. HTTP/3 stubbed (feature-gated) |
| `stormdl-io` | Platform I/O: `WriteBuffer` for coalescing, `TokioBackend` for async file ops. Platform backends stubbed |
| `stormdl-integrity` | BLAKE3 hashing: `IncrementalHasher` for streaming, `verify_content` for validation |
| `stormdl-manifest` | SQLite persistence via `Manifest`. Stores downloads and segments for crash recovery |
| `stormdl-bandwidth` | `RateLimiter` (token bucket), `DownloadQueue` (priority scheduling), `NetworkMonitor` |
| `stormdl-gui` | GPUI + Adabraka UI app. `AppState`, `Download`, channel-based orchestrator communication |

### Key Design Decisions

**Adaptive Segments**: Start with 4 segments based on file size, measure throughput, then converge to optimal count. Rebalance every 500ms by splitting slow segments (<20% of average speed).

**HTTP/2 Awareness**: On HTTP/2, prefer multiplexed streams (1-2 TCP connections) over multiple connections to avoid congestion control competition.

**Write Coalescing**: `WriteBuffer` accumulates data (default 1MB) before flushing to reduce syscall frequency.

**Resume Protocol**: `Manifest` stores per-segment byte ranges and BLAKE3 hashes. On resume, verify hashes, compare server ETag/Last-Modified, continue from last verified offset.

### GUI ↔ Orchestrator Communication

The orchestrator runs on tokio in a separate thread. Communication via `flume` channels:
- `OrchestratorCommand` (GUI → Orchestrator): AddDownload, Pause, Resume, Cancel, SetBandwidthLimit
- `DownloadEvent` (Orchestrator → GUI): ProgressUpdate, SpeedUpdate, StateChange, Complete

Progress updates batched to 30/second max.

## Features

- `default = ["tui"]` - CLI with terminal UI progress
- `gui` - GPUI + Adabraka UI desktop app
- `http3` (stormdl-protocol) - HTTP/3 via quinn (disabled by default, version compat issues)

## Configuration

Default config: `~/.config/storm-dl/config.toml` (see `config/default.toml` for reference)

Key settings:
- `segments.max_segments`: 32 (hard cap)
- `segments.min_segment_size`: 256KB
- `connections.per_host_limit`: 6 (default), 2 for HTTP/2
- `io.write_buffer_size`: 1MB

## CLI Usage

```bash
storm <URL>                    # Basic download
storm <URL> -o ~/Downloads     # Specify output directory
storm <URL> -n file.zip        # Override filename
storm <URL> -s 16              # Use 16 segments
storm <URL> --turbo            # Maximum aggression mode
storm <URL> --checksum <hash>  # Verify BLAKE3 hash after download
```
