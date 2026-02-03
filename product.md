# StormDL — Design Document

**A next-generation download accelerator in Rust**
**Author:** Augustus · **Date:** February 2026 · **Status:** Draft

---

## 1. Motivation

Internet Download Manager (IDM) dominated the 2000s–2010s download landscape with a simple but effective trick: split a file into segments and download them in parallel over multiple HTTP connections. On the congested, lossy networks of that era, this was transformative — often yielding 5–8× speedups over single-connection downloads.

But IDM is a Windows-only, closed-source C++ application stuck in the HTTP/1.1 era. It has no understanding of HTTP/2 multiplexing, HTTP/3 (QUIC), modern congestion control, or zero-copy I/O. Its segment splitting is static and its disk writes are buffered through the Windows API with no awareness of NVMe topology.

StormDL is designed to be the fastest possible download tool on modern hardware and networks, written in Rust for memory safety, fearless concurrency, and zero-cost abstractions.

---

## 2. Design Goals

| Priority | Goal | Metric |
|----------|------|--------|
| P0 | Saturate available bandwidth on any single download | ≥95% of measured link speed |
| P0 | Memory safety — no segfaults, no UB, no data corruption | Zero `unsafe` in application code |
| P1 | Resume any interrupted download seamlessly | 100% resume success for servers supporting Range |
| P1 | Outperform IDM on equivalent hardware and network | ≥1.2× throughput in controlled benchmarks |
| P2 | Minimal resource footprint | <30MB RSS idle, <100MB active with 10 concurrent downloads |
| P2 | Cross-platform (Linux, macOS, Windows) | Single codebase, platform-specific I/O backends |
| P3 | Extensible protocol support | HTTP/1.1, HTTP/2, HTTP/3, FTP, BitTorrent, SFTP |

---

## 3. Why IDM Was Fast (And Where It Falls Short)

Understanding IDM's architecture is essential to surpassing it.

### 3.1 What IDM Got Right

**Multi-segment parallel download.** IDM opens N connections (default 8, configurable up to 32) to the same server, each requesting a different byte range via the `Range` header. On HTTP/1.1, each connection is a separate TCP stream — this effectively bypasses per-connection TCP congestion windows, giving aggregate throughput of N × (single-connection speed). On lossy networks with small TCP windows, this was a massive win.

**Dynamic segment splitting.** If segment A finishes while segment B still has 40% remaining, IDM splits B's remaining range in half and starts a new connection for the second half. This prevents the "one slow segment bottleneck" problem.

**Connection reuse.** IDM keeps connections alive and reuses them across segments, avoiding TCP slow-start penalties on subsequent requests.

**Disk buffering.** IDM writes to a temporary file with pre-allocated size, seeking to the correct offset for each segment. This avoids file fragmentation on NTFS.

### 3.2 Where IDM Falls Short

**HTTP/1.1 only.** IDM doesn't speak HTTP/2 or HTTP/3. On HTTP/2, multiple byte-range requests can be multiplexed over a single TCP connection — you don't need N connections to get N-way parallelism. IDM's multi-connection approach can actually *hurt* on HTTP/2 servers that penalize excessive connections.

**No QUIC/HTTP/3.** QUIC eliminates head-of-line blocking at the transport layer. A single QUIC connection with multiple streams can outperform 8 TCP connections, especially on lossy networks (the exact scenario where IDM's multi-connection approach was designed to help).

**Naive congestion interaction.** Opening 8 TCP connections doesn't magically create 8× bandwidth. Each connection runs its own congestion control (typically CUBIC). They compete with each other and with the user's other traffic. IDM has no global congestion awareness.

**No zero-copy I/O.** IDM reads network data into userspace buffers, then copies it to file I/O buffers. On modern Linux with io_uring, we can do `splice`-like zero-copy from socket to file descriptor.

**No integrity verification.** IDM trusts that bytes arrive correctly. No checksum verification, no content hash validation. Bit rot and corruption go undetected.

**No adaptive algorithm.** Segment count is static. IDM doesn't measure per-connection throughput and dynamically adjust the number of segments based on actual network conditions.

---

## 4. Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                         GUI Layer                        │
│            GPUI (rendering) + Adabraka UI (components)   │
├──────────────────────────────────────────────────────────┤
│                      CLI / TUI                           │
│                   (clap + ratatui)                       │
├──────────────────────────────────────────────────────────┤
│                    Download Orchestrator                  │
│         Scheduling · Queue · Priority · Bandwidth        │
├────────────────┬─────────────────┬───────────────────────┤
│  Segment Mgr   │  Protocol Layer │   Integrity Engine    │
│  Split/Merge   │  HTTP/1.1       │   SHA-256 / BLAKE3    │
│  Dynamic Adj.  │  HTTP/2 (h2)    │   Chunk checksums     │
│  Range Calc    │  HTTP/3 (quinn) │   ETag / Last-Mod     │
│                │  FTP / SFTP     │   Content-Length       │
├────────────────┴─────────────────┴───────────────────────┤
│                   Connection Pool                         │
│         Per-host limits · Keep-alive · TLS session cache  │
├──────────────────────────────────────────────────────────┤
│                     I/O Backend                           │
│     io_uring (Linux) · kqueue (macOS) · IOCP (Windows)   │
│     Pre-allocation · Direct I/O · Write coalescing        │
├──────────────────────────────────────────────────────────┤
│                   Storage Layer                           │
│         Temp files · Atomic rename · Manifest (SQLite)    │
└──────────────────────────────────────────────────────────┘
```

The system is organized into six horizontal layers. Data flows downward (URL → orchestrator → segments → protocol → I/O → disk) and events flow upward (progress, errors, completion).

---

## 5. Core Subsystems

### 5.0 GUI Layer (GPUI + Adabraka UI)

The GUI is the primary interface for most users. We use GPUI as the GPU-accelerated rendering foundation and Adabraka UI as the component library.

**Why GPUI:**
- GPU-accelerated rendering via Metal (macOS), Vulkan (Linux), or DirectX (Windows)
- Immediate-mode-inspired API with retained state — fast iteration, no virtual DOM overhead
- Native Rust, async-first — integrates cleanly with tokio
- Battle-tested at scale in Zed editor
- Sub-millisecond frame times even with complex UIs

**Why Adabraka UI:**
- Your library, your control — no dependency on external component maintainers
- Designed for Rust idioms, not ported from React/web patterns
- Can evolve StormDL-specific components directly in the library

**GUI Architecture:**

```
┌─────────────────────────────────────────────────────────────┐
│                      StormDL App                            │
├─────────────────────────────────────────────────────────────┤
│                     View Layer                              │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ DownloadList│ │ DetailPanel │ │ SettingsView        │   │
│  │   View      │ │   View      │ │                     │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                   Adabraka UI Components                    │
│  ┌────────┐ ┌────────┐ ┌──────────┐ ┌────────┐ ┌────────┐  │
│  │ Button │ │ Table  │ │ Progress │ │ Input  │ │ Modal  │  │
│  └────────┘ └────────┘ └──────────┘ └────────┘ └────────┘  │
├─────────────────────────────────────────────────────────────┤
│                        GPUI                                 │
│         Windows · Events · Layout · GPU Rendering           │
├─────────────────────────────────────────────────────────────┤
│              Platform (Metal / Vulkan / DX)                 │
└─────────────────────────────────────────────────────────────┘
```

**Key Views:**

1. **Main Window** — Split panel layout:
   - Left sidebar: download queue with status icons, drag-to-reorder
   - Center: active downloads with per-segment progress visualization
   - Right panel (collapsible): detailed stats for selected download

2. **Download Item Component** — Shows:
   - Filename + file type icon
   - Overall progress bar (composite of all segments)
   - Per-segment mini-bars (shows which segments are active/complete/stalled)
   - Speed + ETA + downloaded/total bytes
   - Pause/resume/cancel actions

3. **Segment Visualization** — The differentiating UI element:
   - Horizontal bar divided into N segments
   - Each segment colored by state: gray (pending), blue (active), green (complete), red (error), yellow (slow/rebalancing)
   - Animated "flow" effect showing data arriving in real-time
   - Hover to see per-segment stats (speed, bytes, connection info)

4. **Add Download Dialog:**
   - URL input with paste detection (auto-populate from clipboard)
   - Filename override
   - Output directory picker
   - Segment count (auto/manual slider)
   - Bandwidth limit
   - Schedule for later (date/time picker)
   - Custom headers (for authenticated downloads)

5. **Settings Panel:**
   - General: download directory, concurrent downloads, notifications
   - Performance: segment limits, bandwidth, turbo mode toggle
   - Network: proxy settings, per-host connection limits, protocol preferences
   - Advanced: I/O backend selection, direct I/O threshold, integrity verification

**State Management:**

```rust
// Global app state, owned by GPUI app context
struct AppState {
    downloads: Vec<Download>,
    active_download_id: Option<DownloadId>,
    settings: Settings,
    // Channel to send commands to the download orchestrator
    orchestrator_tx: mpsc::Sender<OrchestratorCommand>,
}

// Per-download state, updated by orchestrator via channel
struct Download {
    id: DownloadId,
    url: Url,
    filename: String,
    total_bytes: Option<u64>,
    downloaded_bytes: u64,
    state: DownloadState,
    segments: Vec<SegmentState>,
    speed_samples: RingBuffer<f64>,  // for smoothed speed display
    error: Option<String>,
}

struct SegmentState {
    range: ByteRange,
    downloaded: u64,
    state: SegmentStatus,  // Pending, Active, Complete, Error, Slow
    speed: f64,
}
```

**Orchestrator ↔ GUI Communication:**

The download orchestrator runs on a separate tokio runtime. Communication is via channels:

```rust
// GUI → Orchestrator (commands)
enum OrchestratorCommand {
    AddDownload { url: Url, options: DownloadOptions },
    PauseDownload(DownloadId),
    ResumeDownload(DownloadId),
    CancelDownload(DownloadId),
    SetBandwidthLimit(Option<u64>),
    UpdateSettings(Settings),
}

// Orchestrator → GUI (events)
enum DownloadEvent {
    DownloadAdded { id: DownloadId, info: ResourceInfo },
    ProgressUpdate { id: DownloadId, downloaded: u64, segments: Vec<SegmentState> },
    SpeedUpdate { id: DownloadId, speed: f64 },
    StateChange { id: DownloadId, state: DownloadState },
    SegmentRebalanced { id: DownloadId, old_segments: usize, new_segments: usize },
    Error { id: DownloadId, error: String },
    Complete { id: DownloadId, path: PathBuf, hash: String },
}
```

GPUI's async model lets us poll the event channel on each frame without blocking. Events are batched — the orchestrator sends progress updates at most 30 times per second to avoid overwhelming the UI.

**Adabraka UI Components Needed:**

| Component | Use Case |
|-----------|----------|
| `Button` | Actions (add, pause, resume, cancel, settings) |
| `IconButton` | Toolbar actions with icons |
| `Input` | URL input, filename, search |
| `Table` / `List` | Download queue display |
| `ProgressBar` | Overall + per-segment progress |
| `SegmentedProgress` | Custom: multi-segment visualization (may need to add to Adabraka) |
| `Modal` / `Dialog` | Add download, confirmations |
| `Dropdown` / `Select` | Protocol selection, priority |
| `Slider` | Segment count, bandwidth limit |
| `Toggle` | Turbo mode, settings booleans |
| `Tabs` | Settings categories |
| `Tooltip` | Per-segment stats on hover |
| `ContextMenu` | Right-click actions on downloads |
| `Notification` / `Toast` | Download complete, errors |

**Custom Components to Build:**

1. **SegmentedProgressBar** — The signature StormDL visualization. A horizontal bar divided into N colored segments with animated fill. This might be worth upstreaming to Adabraka UI as it's generally useful.

2. **SpeedGraph** — Mini sparkline showing download speed over time. Optional but nice for the detail panel.

3. **ByteDisplay** — Smart formatting component: "1.2 GB / 4.7 GB" with automatic unit scaling.

### 5.1 Segment Manager

The segment manager is the heart of StormDL's acceleration strategy. It decides how many parallel byte-range requests to issue and dynamically adjusts based on observed throughput.

**Probing phase.** For a new download, we don't immediately open N connections. Instead:

1. Send a HEAD request (or GET with `Range: bytes=0-0`) to determine: total file size, Range support (`Accept-Ranges: bytes`), ETag, Last-Modified, HTTP version.
2. If the server doesn't support Range requests, fall back to single-stream download.
3. If the file is small (<1MB), use a single connection — parallelism overhead isn't worth it.
4. For larger files, start with 4 segments and measure throughput over a 2-second window.

**Adaptive splitting algorithm:**

```
fn optimal_segments(file_size: u64, measured_bw: f64, rtt: Duration) -> usize {
    // BDP = Bandwidth × RTT (Bandwidth-Delay Product)
    let bdp = measured_bw * rtt.as_secs_f64();
    
    // If a single TCP window can't fill the pipe, we need more connections
    // Typical TCP receive window: 64KB default, up to ~4MB with window scaling
    let tcp_window = 65536.0; // conservative estimate
    
    let min_connections = (bdp / tcp_window).ceil() as usize;
    let max_connections = match file_size {
        0..=1_000_000 => 1,
        1_000_001..=10_000_000 => 4,
        10_000_001..=100_000_000 => 8,
        100_000_001..=1_000_000_000 => 16,
        _ => 32,
    };
    
    min_connections.clamp(1, max_connections)
}
```

**Dynamic rebalancing.** Every 500ms, the segment manager evaluates all active segments:

- If a segment's throughput drops below 20% of the average, it is marked "slow."
- If a segment finishes and slow segments exist, the slowest segment's remaining range is split and a new connection is assigned to the second half.
- If all segments are performing equally, no action is taken — unnecessary splits waste connection setup time.
- If the server starts returning 429 (Too Many Requests), reduce the segment count by half and back off.

**HTTP/2 awareness.** On HTTP/2 connections, multiple range requests are multiplexed over a single TCP connection via streams. The segment manager should prefer multiplexed streams over new connections:

- Open 1–2 TCP connections to the server (HTTP/2 typically benefits from at most 2).
- Issue N range requests as separate HTTP/2 streams within those connections.
- This gives the parallelism benefit without the congestion control competition of N independent TCP connections.

### 5.2 Protocol Layer

Each protocol is implemented behind a common `Downloader` trait:

```rust
#[async_trait]
trait Downloader: Send + Sync {
    /// Probe the resource: size, resumability, protocol version, etc.
    async fn probe(&self, url: &Url) -> Result<ResourceInfo>;
    
    /// Download a specific byte range, writing to the provided sink.
    async fn fetch_range(
        &self,
        url: &Url,
        range: ByteRange,
        sink: &dyn AsyncWrite,
        progress: &dyn ProgressReporter,
    ) -> Result<()>;
    
    /// Download the entire resource (for non-resumable downloads).
    async fn fetch_full(
        &self,
        url: &Url,
        sink: &dyn AsyncWrite,
        progress: &dyn ProgressReporter,
    ) -> Result<()>;
}
```

**HTTP/1.1 and HTTP/2:** Use `reqwest` (backed by `hyper`) with connection pooling. `reqwest` handles HTTP version negotiation via ALPN automatically — if the server supports HTTP/2, it will be used.

**HTTP/3 (QUIC):** Use the `quinn` crate for QUIC transport and `h3` for the HTTP/3 framing layer. HTTP/3 is particularly valuable for:
- High-latency connections (satellite, mobile): QUIC's 0-RTT handshake saves a full round trip.
- Lossy networks: QUIC stream multiplexing eliminates head-of-line blocking.
- Connection migration: QUIC connections survive IP address changes (Wi-Fi → cellular).

**Protocol selection strategy:**
1. Attempt HTTP/3 first if the server advertises it via `Alt-Svc` header or DNS HTTPS record.
2. Fall back to HTTP/2 via TLS ALPN negotiation.
3. Fall back to HTTP/1.1 if the server doesn't support HTTP/2.
4. Cache the server's supported protocol for subsequent requests.

**FTP:** Use `suppaftp` with PASV mode and parallel data connections. FTP's REST (restart) command provides native resume support.

**BitTorrent (future):** Integrate `librqbit` or implement a minimal BitTorrent client for magnet links and .torrent files. BitTorrent's piece-based architecture maps naturally to StormDL's segment model.

### 5.3 Connection Pool

The connection pool manages TCP/QUIC connections and enforces per-host limits to avoid being throttled or blocked.

**Per-host connection limits:**
- Default: 6 connections per host (matches browser behavior).
- HTTP/2 hosts: 2 connections (multiplexing makes more unnecessary).
- Configurable by the user, with a hard cap of 32.

**TLS session caching:** Cache TLS sessions (session tickets and PSK identities) to enable 0-RTT resumption on subsequent connections to the same host. This is critical for segmented downloads — after the first segment establishes TLS, subsequent segments skip the full handshake.

**Connection health monitoring:** Track per-connection metrics (throughput, error rate, latency). Connections with degrading performance are closed and replaced rather than left to drag down the overall download.

### 5.4 I/O Backend

Disk I/O is often the bottleneck on fast networks (1+ Gbps). StormDL uses platform-specific I/O backends to minimize copies and syscall overhead.

**Linux: io_uring**

`io_uring` is the highest-performance I/O interface on Linux. StormDL uses it for:
- **Async file writes** without thread pool overhead (unlike tokio's `spawn_blocking` approach).
- **Write coalescing:** Buffer incoming network data and submit batched write operations to the ring, reducing syscall frequency.
- **Fixed buffers:** Register a set of I/O buffers with the kernel to eliminate per-syscall buffer registration overhead.
- **Pre-allocation:** Use `fallocate` to pre-allocate the full file size before writing segments, eliminating filesystem metadata updates during the download and reducing fragmentation.

Use the `tokio-uring` or `io-uring` crate. Target kernel 5.11+ for best feature support.

**macOS: kqueue + pwritev**

macOS doesn't have io_uring. Use:
- `kqueue` for async event notification (via tokio/mio).
- `pwritev` for scatter-gather writes at specific offsets without seeking.
- `F_PREALLOCATE` + `ftruncate` for file pre-allocation.
- `F_NOCACHE` (fcntl) for direct I/O to bypass the VFS cache on large downloads — the data won't be re-read soon, so caching wastes memory.

**Windows: IOCP**

- IOCP (I/O Completion Ports) via `tokio` (which uses mio → wepoll on Windows).
- `SetFileValidData` for fast pre-allocation without zeroing.
- `FILE_FLAG_NO_BUFFERING` for direct I/O on large downloads.

**Write coalescing strategy:**

Network data arrives in small chunks (typically 16–64KB from TLS records). Writing each chunk individually to disk would generate excessive syscalls. Instead:

1. Each segment maintains a 1MB write buffer.
2. Incoming data is appended to the buffer.
3. When the buffer is full (or a flush interval of 200ms elapses), the entire buffer is written in one operation.
4. On io_uring, multiple segments' buffers can be submitted in a single `io_uring_enter` call.

### 5.5 Integrity Engine

Every download must be verifiable. StormDL provides multiple layers of integrity checking.

**Transport-level:** TLS ensures data integrity in transit. For non-TLS connections (plain HTTP, FTP), we compute a running checksum.

**Chunk-level checksums:** As each segment is written to disk, a BLAKE3 hash is computed incrementally over the written data. BLAKE3 is chosen over SHA-256 because it is 4–5× faster on modern CPUs with SIMD (AVX-512, NEON) and supports incremental hashing natively.

**Content verification:** If the server provides a `Content-MD5` header, `Digest` header (RFC 3230), or the download URL includes a hash (common for package managers), verify the final file against it.

**Manifest:** Each download's progress is tracked in a manifest file (SQLite database) that records: per-segment byte ranges completed, per-segment BLAKE3 hashes, overall file metadata (size, ETag, Last-Modified), and download state (in-progress, paused, complete, failed). This enables resume after crash — on restart, read the manifest, verify completed segment hashes, and resume only the incomplete segments.

### 5.6 Storage Layer

**Temporary files:** Each download writes to a `.storm` temporary file alongside a `.storm-manifest` SQLite database. The temp file is pre-allocated to the full size and segments write to their respective byte offsets.

**Atomic completion:** When all segments are complete and verified, the temp file is renamed to the final filename via `rename()` (atomic on POSIX). The manifest is deleted.

**Crash recovery flow:**
1. On startup, scan the download directory for `.storm-manifest` files.
2. For each manifest, verify which segments completed successfully (check BLAKE3 hashes).
3. Resume incomplete segments from their last verified offset.
4. If the server's ETag or Last-Modified has changed, discard all progress and restart.

---

## 6. Bandwidth Management

### 6.1 Global Rate Limiter

A token-bucket rate limiter provides global bandwidth control:
- The user can set a global bandwidth limit (e.g., 10 MB/s) and per-download limits.
- Tokens are distributed to active downloads proportionally to their configured priority.
- When no limit is set, the limiter is completely bypassed (zero overhead).

### 6.2 Congestion Awareness

StormDL should be a good network citizen. Rather than blindly opening 32 connections and saturating the user's uplink with ACKs:
- Monitor the user's overall network throughput (via `/proc/net/dev` on Linux, `getifaddrs` on macOS).
- If the user is actively browsing or streaming, automatically reduce download aggressiveness.
- Provide a "turbo" mode that disables politeness and maximizes throughput.

### 6.3 Scheduling and Queuing

- Downloads are queued with configurable concurrency (default: 3 simultaneous downloads).
- Priority levels: Critical, High, Normal, Low, Background.
- Background downloads automatically reduce segment count and bandwidth allocation.
- Schedule downloads for specific times (e.g., "download overnight when bandwidth is cheaper").

---

## 7. Resume Protocol

Robust resume is non-negotiable. Here is the exact flow:

```
RESUME FLOW
──────────

1. Read .storm-manifest for download D
2. For each segment S in manifest:
   a. Read S.completed_bytes, S.blake3_hash
   b. Verify hash of bytes [S.start .. S.start + S.completed_bytes] on disk
   c. If hash matches → mark segment as "partially complete, resume from S.start + S.completed_bytes"
   d. If hash doesn't match → mark segment as "corrupted, restart from S.start"
3. Send HEAD request to server
   a. Compare ETag and Last-Modified with manifest
   b. If match → proceed with resume
   c. If different → file changed on server, discard all progress, restart
4. For each segment needing work:
   a. Send GET with Range: bytes=(S.start + S.completed_bytes)-(S.end)
   b. Verify server returns 206 Partial Content
   c. Resume writing to temp file at correct offset
```

---

## 8. Crate Dependency Map

| Subsystem | Crate | Purpose |
|-----------|-------|---------|
| Async runtime | `tokio` | Multi-threaded async executor |
| GUI framework | `gpui` | GPU-accelerated UI rendering |
| UI components | `adabraka-ui` | Rust-native component library |
| HTTP/1.1 + HTTP/2 | `reqwest` / `hyper` | HTTP client with connection pooling |
| HTTP/3 | `quinn` + `h3` | QUIC transport + HTTP/3 framing |
| TLS | `rustls` | Pure-Rust TLS (no OpenSSL dependency) |
| DNS | `hickory-dns` | Async DNS resolution with caching |
| io_uring | `tokio-uring` or `io-uring` | Linux async I/O |
| Hashing | `blake3` | SIMD-accelerated incremental hashing |
| Database | `rusqlite` | Manifest storage |
| CLI | `clap` | Argument parsing |
| TUI | `ratatui` | Terminal UI for progress display |
| Rate limiting | `governor` | Token-bucket rate limiter |
| FTP | `suppaftp` | FTP/FTPS client |
| Logging | `tracing` | Structured, async-aware logging |
| Error handling | `thiserror` + `anyhow` | Library/application error types |
| Serialization | `serde` + `serde_json` | Config and manifest serialization |
| Channels | `flume` | MPMC channels for GUI ↔ orchestrator |

---

## 9. Project Structure

```
storm-dl/
├── Cargo.toml
├── crates/
│   ├── storm-core/          # Core types, traits, error types
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── types.rs     # ByteRange, ResourceInfo, DownloadState
│   │       ├── traits.rs    # Downloader, ProgressReporter, IoBackend
│   │       └── error.rs
│   ├── storm-segment/       # Segment manager + adaptive algorithm
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── manager.rs   # SegmentManager
│   │       ├── splitter.rs  # Adaptive splitting logic
│   │       └── rebalancer.rs
│   ├── storm-protocol/      # Protocol implementations
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── http.rs      # HTTP/1.1 + HTTP/2 via reqwest
│   │       ├── h3.rs        # HTTP/3 via quinn
│   │       ├── ftp.rs
│   │       └── pool.rs      # Connection pool management
│   ├── storm-io/            # Platform I/O backends
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── uring.rs     # Linux io_uring backend
│   │       ├── kqueue.rs    # macOS backend
│   │       ├── iocp.rs      # Windows backend
│   │       └── coalesce.rs  # Write coalescing buffer
│   ├── storm-integrity/     # Hashing and verification
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── hasher.rs    # BLAKE3 incremental hasher
│   │       └── verify.rs    # Content verification
│   ├── storm-manifest/      # Download state persistence
│   │   └── src/
│   │       ├── lib.rs
│   │       └── db.rs        # SQLite manifest operations
│   ├── storm-bandwidth/     # Rate limiting and scheduling
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── limiter.rs   # Token-bucket rate limiter
│   │       ├── scheduler.rs # Download queue and priority
│   │       └── monitor.rs   # Network throughput monitor
│   └── storm-gui/           # GPUI + Adabraka UI application
│       └── src/
│           ├── lib.rs
│           ├── app.rs           # GPUI app setup, main window
│           ├── state.rs         # AppState, Download, SegmentState
│           ├── views/
│           │   ├── mod.rs
│           │   ├── main_window.rs
│           │   ├── download_list.rs
│           │   ├── download_item.rs
│           │   ├── detail_panel.rs
│           │   ├── add_download.rs
│           │   └── settings.rs
│           ├── components/
│           │   ├── mod.rs
│           │   ├── segmented_progress.rs  # Custom multi-segment bar
│           │   ├── speed_graph.rs         # Sparkline speed visualization
│           │   └── byte_display.rs        # Smart byte formatting
│           └── theme.rs         # StormDL theming on top of Adabraka
├── src/
│   ├── main.rs              # Entry point: CLI vs GUI dispatch
│   ├── cli.rs               # CLI implementation
│   ├── tui.rs               # Terminal UI (ratatui)
│   └── orchestrator.rs      # Download orchestrator (shared by CLI/TUI/GUI)
├── tests/
│   ├── integration/
│   │   ├── resume_test.rs
│   │   ├── segment_test.rs
│   │   └── protocol_test.rs
│   └── bench/
│       ├── throughput_bench.rs
│       └── io_bench.rs
└── config/
    └── default.toml
```

Workspace with separate crates enforces clean dependency boundaries. `storm-core` has zero dependencies — it defines only types and traits. Each subsystem crate depends only on `storm-core` plus its specific external dependencies.

The `storm-gui` crate is intentionally separate from `storm-core` — it depends on GPUI and Adabraka UI, which are heavyweight. Users who only want the CLI/TUI can build without the `gui` feature to avoid pulling in GPU rendering dependencies.

---

## 10. Performance Targets and Benchmarks

### 10.1 Test Methodology

All benchmarks should be run against a controlled test server (nginx or caddy) on localhost to eliminate network variability, plus real-world tests against CDNs (Cloudflare, Fastly, AWS CloudFront).

### 10.2 Targets

| Scenario | Target | IDM Baseline |
|----------|--------|-------------|
| 1 Gbps LAN, 1GB file | ≥118 MB/s (≥95% wire speed) | ~100 MB/s |
| 100 Mbps WAN, 100ms RTT, 500MB file | ≥11.5 MB/s | ~9 MB/s |
| Lossy network (2% packet loss), 100MB file | ≥70% of clean throughput | ~40% |
| Resume after crash at 50% | <2s to resume | ~5s |
| 10 concurrent downloads, 10 Gbps | ≥1 GB/s aggregate | N/A (IDM struggles) |
| Memory usage, 10 active downloads | <100 MB RSS | ~150 MB |
| Cold start to first byte | <200ms | ~500ms |

### 10.3 Profiling Strategy

- **Network throughput:** `perf` + custom metrics emitted via `tracing`.
- **I/O throughput:** `blktrace` / `iostat` to verify write coalescing effectiveness.
- **Memory:** `DHAT` (via `dhat-rs`) for heap profiling, watching for buffer bloat.
- **CPU:** `flamegraph` (via `cargo-flamegraph`) to find hot paths — TLS decryption and hashing will dominate; verify SIMD is being used.
- **Latency:** Measure time from "URL submitted" to "first byte written to disk" for each protocol.

---

## 11. Configuration

```toml
# ~/.config/storm-dl/config.toml

[general]
download_dir = "~/Downloads"
max_concurrent_downloads = 3
temp_extension = ".storm"

[segments]
min_segments = 1
max_segments = 32
initial_segments = 4           # starting point before adaptive kicks in
min_segment_size = "256KB"     # don't split below this
rebalance_interval_ms = 500
slow_threshold_pct = 20        # segment is "slow" if below 20% of average

[connections]
per_host_limit = 6
per_host_limit_h2 = 2
connect_timeout_ms = 5000
read_timeout_ms = 30000
tls_session_cache = true
prefer_h3 = true               # try HTTP/3 first if available

[bandwidth]
global_limit = "0"             # 0 = unlimited
per_download_limit = "0"
polite_mode = true             # reduce aggression when other traffic detected
turbo_mode = false             # override polite_mode

[io]
write_buffer_size = "1MB"
flush_interval_ms = 200
direct_io_threshold = "10MB"   # use direct I/O for files larger than this
preallocate = true

[integrity]
verify_checksums = true
hash_algorithm = "blake3"

[resume]
manifest_db = true
verify_on_resume = true
```

---

## 12. CLI Interface Design

```
storm-dl — the fastest download tool

USAGE:
    storm <URL> [OPTIONS]
    storm batch <FILE> [OPTIONS]
    storm resume [OPTIONS]
    storm list [OPTIONS]

EXAMPLES:
    storm https://example.com/file.zip
    storm https://example.com/file.iso -s 16 -o ~/ISOs/
    storm https://example.com/file.tar.gz --limit 5MB/s
    storm batch urls.txt --concurrent 5
    storm resume --all
    storm list --active

OPTIONS:
    -o, --output <DIR>         Output directory [default: ~/Downloads]
    -n, --name <NAME>          Override output filename
    -s, --segments <N>         Number of segments [default: auto]
    -c, --concurrent <N>       Max concurrent downloads [default: 3]
    -l, --limit <RATE>         Bandwidth limit (e.g., 10MB/s)
    -H, --header <K:V>         Custom request header (repeatable)
        --turbo                Maximum aggression mode
        --no-resume            Don't save resume manifest
        --checksum <HASH>      Verify file against hash after download
        --http1                Force HTTP/1.1
        --http2                Force HTTP/2
        --http3                Force HTTP/3
    -q, --quiet                Suppress progress output
    -v, --verbose              Detailed logging
```

The TUI (via `ratatui`) shows real-time per-segment progress bars, per-segment throughput, overall speed, ETA, and a bandwidth graph.

---

## 13. Phased Implementation Roadmap

### Phase 1 — Core Pipeline (Weeks 1–3)

Deliver a working single-file downloader that already beats naive `wget`/`curl`.

- [ ] `storm-core`: Define types and traits
- [ ] `storm-protocol/http`: HTTP/1.1 + HTTP/2 via reqwest with Range support
- [ ] `storm-segment`: Basic static segment splitting (no adaptive yet)
- [ ] `storm-io`: Tokio file I/O (no platform-specific backends yet)
- [ ] `storm-manifest`: SQLite manifest for resume
- [ ] CLI: Basic `storm <URL>` with progress bar
- [ ] Integration test: download a 100MB file from localhost nginx, verify correctness

**Exit criteria:** Download a 1GB file over localhost at ≥80% of disk write speed. Resume works after kill -9.

### Phase 2 — Adaptive Performance (Weeks 4–6)

Make it smart.

- [ ] Adaptive segment count based on measured throughput and RTT
- [ ] Dynamic segment rebalancing (split slow segments)
- [ ] HTTP/2 stream multiplexing (prefer streams over connections)
- [ ] Write coalescing buffers (1MB per segment)
- [ ] Connection pool with per-host limits and TLS session caching
- [ ] Bandwidth rate limiter
- [ ] BLAKE3 integrity verification

**Exit criteria:** Consistently outperform IDM on controlled benchmarks. Adaptive algorithm converges to optimal segment count within 5 seconds.

### Phase 3 — Platform I/O (Weeks 7–9)

Squeeze out every last bit of throughput.

- [ ] io_uring backend for Linux
- [ ] kqueue + pwritev backend for macOS  
- [ ] IOCP backend for Windows
- [ ] Direct I/O for large files
- [ ] File pre-allocation on all platforms
- [ ] Zero-copy where possible (splice on Linux)

**Exit criteria:** ≥95% wire speed on 1 Gbps LAN. I/O is not the bottleneck.

### Phase 4 — GUI Foundation (Weeks 10–12)

Build the GPUI + Adabraka UI application shell.

- [ ] `storm-gui` crate setup with GPUI
- [ ] Main window layout (sidebar + center + detail panel)
- [ ] Download list view with basic item rendering
- [ ] Add download dialog
- [ ] Orchestrator ↔ GUI channel communication
- [ ] Basic progress bars (overall, not per-segment yet)
- [ ] Pause/resume/cancel actions
- [ ] Settings panel (read/write config.toml)

**Exit criteria:** Functional GUI that can add downloads, show progress, and persist settings. Not pretty yet, but works.

### Phase 5 — GUI Polish + Visualization (Weeks 13–15)

The signature StormDL experience.

- [ ] `SegmentedProgressBar` component — multi-segment visualization with state colors
- [ ] Animated data flow effect (bytes arriving in real-time)
- [ ] Per-segment tooltips (speed, bytes, connection info)
- [ ] `SpeedGraph` sparkline component
- [ ] Download queue drag-to-reorder
- [ ] Context menus (right-click actions)
- [ ] Notifications / toasts (download complete, errors)
- [ ] Keyboard shortcuts
- [ ] StormDL theming (dark mode default, light mode option)
- [ ] App icon and branding

**Exit criteria:** GUI is visually distinctive and pleasant to use. The segment visualization is the "wow" feature that differentiates StormDL.

### Phase 6 — Extended Protocols & Browser Integration (Weeks 16–18)

- [ ] HTTP/3 via quinn
- [ ] FTP support
- [ ] Batch downloads from URL list
- [ ] Download scheduling (time-based)
- [ ] Polite mode (network traffic awareness)
- [ ] Browser extension for link capture (Chrome/Firefox)
- [ ] Native messaging host for extension ↔ app communication
- [ ] System tray / menu bar integration (minimize to tray)
- [ ] Comprehensive benchmark suite

**Exit criteria:** Feature-complete v1.0 release candidate. Browser extension captures links and sends to StormDL.

### Phase 7 — Platform Distribution (Weeks 19–20)

- [ ] macOS: DMG installer, code signing, notarization
- [ ] Windows: MSI/NSIS installer, code signing
- [ ] Linux: AppImage, .deb, .rpm, Flatpak
- [ ] Auto-update mechanism
- [ ] Crash reporting (opt-in)
- [ ] Landing page and documentation site

**Exit criteria:** Users can download and install StormDL on any major platform with a single click.

---

## 14. Open Questions

1. **Should we implement BitTorrent in v1?** The segment model maps well to torrent pieces, but it's a significant implementation effort. Consider integrating `librqbit` as a crate dependency rather than writing our own.

2. **GPUI async runtime integration.** GPUI has its own async executor. The download orchestrator runs on tokio. Need to verify clean interop — likely use channels (`flume`) at the boundary rather than trying to share runtimes. May need to spawn tokio runtime in a separate thread.

3. **Adabraka UI component gaps.** Audit Adabraka UI against the component list in Section 5.0. Missing components (SegmentedProgressBar, SpeedGraph) should be built as StormDL-specific first, then upstreamed to Adabraka if they're generally useful.

4. **Browser integration architecture.** A Chrome/Firefox extension that intercepts download links and passes them to StormDL via a native messaging host. This was IDM's killer feature. Needs a local IPC mechanism — Unix domain socket on POSIX, named pipe on Windows.

5. **Mirroring.** IDM doesn't support downloading the same file from multiple mirrors simultaneously. This is common in Linux package managers (apt uses mirrors). Should StormDL support multi-source downloads where different segments come from different mirrors?

6. **Memory-mapped I/O vs. explicit writes.** `mmap` + `madvise(MADV_SEQUENTIAL)` is simpler code but gives up control over write timing and makes error handling harder (SIGBUS on disk full). Current design favors explicit writes. Revisit if benchmarks show mmap is faster.

7. **GPUI Windows support.** GPUI's Windows backend is less mature than macOS. Need to validate early in Phase 4 that the GUI works acceptably on Windows, or plan for platform-specific fallbacks.

8. **System tray behavior.** Should StormDL minimize to system tray by default? This is expected for download managers but can annoy users who prefer apps to actually quit. Make it configurable, default to tray on Windows, dock on macOS.

---

## 15. Competitive Landscape

| Tool | Language | Segments | HTTP/2 | HTTP/3 | Adaptive | Resume | GUI | Platform |
|------|----------|----------|--------|--------|----------|--------|-----|----------|
| IDM | C++ | Yes (32) | No | No | No | Yes | Yes (Win32) | Windows |
| aria2 | C++ | Yes (16) | No | No | No | Yes | No (CLI) | Cross |
| axel | C | Yes (10) | No | No | No | Yes | No (CLI) | Linux/macOS |
| wget2 | C | Yes | Yes | No | No | Yes | No (CLI) | Cross |
| curl | C | No | Yes | Yes | N/A | Yes | No (CLI) | Cross |
| **StormDL** | **Rust** | **Yes (32)** | **Yes** | **Yes** | **Yes** | **Yes** | **Yes (GPUI)** | **Cross** |

StormDL's key differentiators:
- **Adaptive segment management** — converges to optimal parallelism automatically
- **HTTP/3 support** — first download manager to leverage QUIC's stream multiplexing
- **Platform-optimized I/O** — io_uring on Linux, proper async I/O everywhere
- **Memory safety** — Rust guarantees no buffer overflows, no data races
- **Modern GPU-accelerated GUI** — GPUI + Adabraka UI for a responsive, cross-platform interface
- **Distinctive visualization** — per-segment progress with real-time data flow animation
