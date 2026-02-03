mod cli;
mod orchestrator;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "storm")]
#[command(author, version, about = "StormDL — the fastest download tool")]
struct Args {
    #[arg(help = "URL to download")]
    url: Option<String>,

    #[arg(short, long, help = "Output directory")]
    output: Option<String>,

    #[arg(short, long, help = "Override output filename")]
    name: Option<String>,

    #[arg(short, long, help = "Number of segments (default: auto)")]
    segments: Option<usize>,

    #[arg(short, long, default_value = "3", help = "Max concurrent downloads")]
    concurrent: usize,

    #[arg(short, long, help = "Bandwidth limit (e.g., 10MB/s)")]
    limit: Option<String>,

    #[arg(long, help = "Conservative mode for sensitive servers")]
    gentle: bool,

    #[arg(long, help = "Don't save resume manifest")]
    no_resume: bool,

    #[arg(long, help = "Verify file against hash after download")]
    checksum: Option<String>,

    #[arg(long, help = "Force HTTP/1.1")]
    http1: bool,

    #[arg(long, help = "Force HTTP/2")]
    http2: bool,

    #[arg(long, help = "Force HTTP/3")]
    http3: bool,

    #[arg(long = "mirror", short = 'm', help = "Additional mirror URLs")]
    mirrors: Vec<String>,

    #[arg(short, long, help = "Suppress progress output")]
    quiet: bool,

    #[arg(short, long, help = "Detailed logging")]
    verbose: bool,

    #[cfg(feature = "gui")]
    #[arg(long, help = "Launch GUI")]
    gui: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let filter = if args.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    #[cfg(feature = "gui")]
    if args.gui || args.url.is_none() {
        return run_gui();
    }

    #[cfg(not(feature = "gui"))]
    if args.url.is_none() {
        eprintln!("Usage: storm <URL> [OPTIONS]");
        eprintln!("       storm --help for more information");
        std::process::exit(1);
    }

    if let Some(url) = args.url {
        cli::download(&url, cli::DownloadArgs {
            output: args.output,
            name: args.name,
            segments: args.segments,
            limit: args.limit,
            turbo: !args.gentle,
            no_resume: args.no_resume,
            checksum: args.checksum,
            quiet: args.quiet,
            mirrors: args.mirrors,
        })?;
    }

    Ok(())
}

#[cfg(feature = "gui")]
fn run_gui() -> Result<()> {
    let (cmd_tx, cmd_rx) = flume::unbounded();
    let (event_tx, event_rx) = flume::unbounded();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            orchestrator::run(cmd_rx, event_tx).await;
        });
    });

    storm_gui::run_app(cmd_tx, event_rx);
    Ok(())
}
