use anyhow::Result;
use clap::Parser;
use indexmap::IndexSet;
use ip_extract::ExtractorBuilder;
use memmap2::Mmap;
use rayon::prelude::*;
use ripline::{
    line_buffer::{LineBufferBuilder, LineBufferReader},
    lines::LineIter,
};
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[clap(
    name = "justips",
    about = "Blazing fast IP address extraction",
    version
)]
struct Args {
    /// Input file(s) to process. Leave empty or use "-" to read from stdin
    #[clap(value_name = "FILE")]
    input: Vec<PathBuf>,

    /// Deduplicate IPs (unordered, fastest)
    #[clap(short, long, conflicts_with = "unique_ordered")]
    unique: bool,

    /// Deduplicate IPs, preserving first-seen order
    #[clap(short = 'U', long, conflicts_with = "unique")]
    unique_ordered: bool,

    /// Include all types of IP addresses
    #[clap(long)]
    all: bool,

    /// Exclude private IP addresses
    #[clap(long)]
    no_private: bool,

    /// Exclude loopback IP addresses
    #[clap(long)]
    no_loopback: bool,

    /// Exclude broadcast/link-local IP addresses
    #[clap(long)]
    no_broadcast: bool,
}

/// Target chunk size for parallel work units (~4MB).
/// Actual boundaries are snapped to the next newline for correctness.
const TARGET_CHUNK_SIZE: usize = 4 * 1024 * 1024;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Broken pipe is normal (e.g. `justips file | head`)
            if err
                .chain()
                .any(|e| matches!(e.downcast_ref::<io::Error>(), Some(e) if e.kind() == io::ErrorKind::BrokenPipe))
            {
                return ExitCode::SUCCESS;
            }
            eprintln!("Error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args = Args::parse();
    if args.input.is_empty() {
        args.input.push(PathBuf::from("-"));
    }

    let extractor = build_extractor(&args)?;

    if args.unique {
        // Unordered dedup: per-chunk HashSets merged. Fastest unique mode.
        let mut seen: HashSet<String> = HashSet::new();
        for path in &args.input {
            if path.as_os_str() == "-" {
                dedup_stdin(&extractor, &mut seen)?;
            } else {
                dedup_file(path, &extractor, &mut seen)?;
            }
        }

        let mut out = io::BufWriter::with_capacity(128 * 1024, io::stdout());
        for ip in &seen {
            out.write_all(ip.as_bytes())?;
            out.write_all(b"\n")?;
        }
        out.flush()?;
    } else if args.unique_ordered {
        // Ordered dedup: per-chunk IndexSets merged, preserving first-seen order.
        let mut seen: IndexSet<String> = IndexSet::new();
        for path in &args.input {
            if path.as_os_str() == "-" {
                dedup_ordered_stdin(&extractor, &mut seen)?;
            } else {
                dedup_ordered_file(path, &extractor, &mut seen)?;
            }
        }

        let mut out = io::BufWriter::with_capacity(128 * 1024, io::stdout());
        for ip in &seen {
            out.write_all(ip.as_bytes())?;
            out.write_all(b"\n")?;
        }
        out.flush()?;
    } else {
        // Stream directly — no memory overhead.
        for path in &args.input {
            if path.as_os_str() == "-" {
                stream_stdin(&extractor)?;
            } else {
                stream_file(path, &extractor)?;
            }
        }
    }

    Ok(())
}

fn build_extractor(args: &Args) -> Result<ip_extract::Extractor> {
    let include_private = args.all || !args.no_private;
    let include_loopback = args.all || !args.no_loopback;
    let include_broadcast = args.all || !args.no_broadcast;

    let mut builder = ExtractorBuilder::new();
    if !include_private {
        builder.ignore_private();
    }
    if !include_loopback {
        builder.ignore_loopback();
    }
    if !include_broadcast {
        builder.ignore_broadcast();
    }
    Ok(builder.build()?)
}

// ---------------------------------------------------------------------------
// Streaming output (no post-processing needed)
// ---------------------------------------------------------------------------

/// Split the mmap into chunks whose boundaries fall on newline characters.
fn split_at_newlines(data: &[u8], target: usize) -> Vec<(usize, usize)> {
    let len = data.len();
    if len == 0 {
        return vec![];
    }

    let mut chunks = Vec::with_capacity(len / target + 1);
    let mut start = 0;

    while start < len {
        let ideal_end = std::cmp::min(start + target, len);
        let end = if ideal_end >= len {
            len
        } else {
            match memchr::memchr(b'\n', &data[ideal_end..]) {
                Some(offset) => ideal_end + offset + 1,
                None => len,
            }
        };
        chunks.push((start, end));
        start = end;
    }
    chunks
}

fn stream_file(path: &PathBuf, extractor: &ip_extract::Extractor) -> Result<()> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Ok(());
    }

    let mmap = unsafe { Mmap::map(&file)? };
    let chunks = split_at_newlines(&mmap, TARGET_CHUNK_SIZE);

    let results: Vec<Vec<u8>> = chunks
        .into_par_iter()
        .map(|(start, end)| {
            let slice = &mmap[start..end];
            let mut buf = Vec::with_capacity(16 * 1024);
            for m in extractor.match_iter(slice) {
                buf.extend_from_slice(m.as_str().as_bytes());
                buf.push(b'\n');
            }
            buf
        })
        .collect();

    let mut out = io::BufWriter::with_capacity(128 * 1024, io::stdout());
    for buf in &results {
        out.write_all(buf)?;
    }
    out.flush()?;
    Ok(())
}

fn stream_stdin(extractor: &ip_extract::Extractor) -> Result<()> {
    let mut out = io::BufWriter::with_capacity(64 * 1024, io::stdout());
    let mut line_buffer = LineBufferBuilder::new().capacity(64 * 1024).build();
    let reader = io::stdin();
    let mut lb_reader = LineBufferReader::new(reader, &mut line_buffer);

    while lb_reader.fill()? {
        let buffer = lb_reader.buffer();
        for line in LineIter::new(b'\n', buffer) {
            for m in extractor.match_iter(line) {
                out.write_all(m.as_str().as_bytes())?;
                out.write_all(b"\n")?;
            }
        }
        lb_reader.consume_all();
    }
    out.flush()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Unordered dedup (for -u/--unique)
// ---------------------------------------------------------------------------

/// Per-chunk parallel dedup, then merge into the global set.
fn dedup_file(
    path: &PathBuf,
    extractor: &ip_extract::Extractor,
    seen: &mut HashSet<String>,
) -> Result<()> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Ok(());
    }

    let mmap = unsafe { Mmap::map(&file)? };
    let chunks = split_at_newlines(&mmap, TARGET_CHUNK_SIZE);

    let per_chunk: Vec<HashSet<String>> = chunks
        .into_par_iter()
        .map(|(start, end)| {
            let slice = &mmap[start..end];
            extractor
                .match_iter(slice)
                .map(|m| m.as_str().into_owned())
                .collect()
        })
        .collect();

    for chunk_set in per_chunk {
        seen.extend(chunk_set);
    }
    Ok(())
}

/// Streaming dedup for stdin — check-and-insert inline, no output until done.
fn dedup_stdin(extractor: &ip_extract::Extractor, seen: &mut HashSet<String>) -> Result<()> {
    let mut line_buffer = LineBufferBuilder::new().capacity(64 * 1024).build();
    let reader = io::stdin();
    let mut lb_reader = LineBufferReader::new(reader, &mut line_buffer);

    while lb_reader.fill()? {
        let buffer = lb_reader.buffer();
        for line in LineIter::new(b'\n', buffer) {
            for m in extractor.match_iter(line) {
                seen.insert(m.as_str().into_owned());
            }
        }
        lb_reader.consume_all();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Ordered dedup (for -U/--unique-ordered)
// ---------------------------------------------------------------------------

/// Per-chunk parallel dedup with IndexSet, then merge preserving global order.
fn dedup_ordered_file(
    path: &PathBuf,
    extractor: &ip_extract::Extractor,
    seen: &mut IndexSet<String>,
) -> Result<()> {
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Ok(());
    }

    let mmap = unsafe { Mmap::map(&file)? };
    let chunks = split_at_newlines(&mmap, TARGET_CHUNK_SIZE);

    // Each chunk produces an IndexSet (deduped, insertion-ordered).
    // Rayon's indexed par_iter preserves chunk order in the outer Vec,
    // so extending the global set in order preserves first-seen semantics.
    let per_chunk: Vec<IndexSet<String>> = chunks
        .into_par_iter()
        .map(|(start, end)| {
            let slice = &mmap[start..end];
            extractor
                .match_iter(slice)
                .map(|m| m.as_str().into_owned())
                .collect()
        })
        .collect();

    for chunk_set in per_chunk {
        seen.extend(chunk_set);
    }
    Ok(())
}

/// Streaming ordered dedup for stdin — insert inline, IndexSet preserves order.
fn dedup_ordered_stdin(
    extractor: &ip_extract::Extractor,
    seen: &mut IndexSet<String>,
) -> Result<()> {
    let mut line_buffer = LineBufferBuilder::new().capacity(64 * 1024).build();
    let reader = io::stdin();
    let mut lb_reader = LineBufferReader::new(reader, &mut line_buffer);

    while lb_reader.fill()? {
        let buffer = lb_reader.buffer();
        for line in LineIter::new(b'\n', buffer) {
            for m in extractor.match_iter(line) {
                seen.insert(m.as_str().into_owned());
            }
        }
        lb_reader.consume_all();
    }
    Ok(())
}
