use anyhow::{Error, Result};
use camino::Utf8PathBuf;
use clap::{Parser, ValueEnum};
use ripline::{
    line_buffer::{LineBufferBuilder, LineBufferReader},
    lines::LineIter,
};
use rustc_hash::FxHashMap as HashMap;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use termcolor::{ColorChoice, StandardStream};

// Use modules from the library instead of redefining them
use geoipsed::{files, geoip, input, mmdb, ExtractorBuilder, Tag, Tagged};
use input::FileOrStdin;

/// Check if the error chain contains a broken pipe error.
#[inline(always)]
fn is_broken_pipe(err: &Error) -> bool {
    // Look for a broken pipe error in the error chain
    for cause in err.chain() {
        if let Some(io_err) = cause.downcast_ref::<io::Error>() {
            if io_err.kind() == io::ErrorKind::BrokenPipe {
                return true;
            }
        }
    }
    false
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Show only nonempty parts of lines that match
    #[clap(short, long)]
    only_matching: bool,

    /// Use markers to highlight the matching strings
    #[clap(short = 'C', long, value_enum, default_value_t = ArgsColorChoice::Auto)]
    color: ArgsColorChoice,

    /// Specify the format of the IP address decoration. Use the --list-templates option
    /// to see which fields are available. Field names are enclosed in {}, for example
    /// "{field1} any fixed string {field2} & {field3}"
    #[clap(short, long)]
    template: Option<String>,

    /// Output matches as JSON with tag information for each line
    #[clap(long, conflicts_with = "only_matching")]
    tag: bool,

    /// Output matches as JSON with tag information for entire files
    #[clap(long, conflicts_with = "only_matching")]
    tag_files: bool,

    /// Extract IPs only without MMDB lookups or templating (fast path, implies --only-matching)
    #[clap(short = 'j', long, conflicts_with_all = &["template", "tag", "tag_files", "provider", "include", "list_providers", "list_templates", "only_routable", "only_matching"])]
    justips: bool,

    /// Include all types of IP addresses in matches
    #[clap(long)]
    all: bool,

    /// Exclude private IP addresses from matches
    #[clap(long)]
    no_private: bool,

    /// Exclude loopback IP addresses from matches
    #[clap(long)]
    no_loopback: bool,

    /// Exclude broadcast/link-local IP addresses from matches
    #[clap(long)]
    no_broadcast: bool,

    /// Only include internet-routable IP addresses (requires valid ASN entry)
    #[clap(long)]
    only_routable: bool,

    /// Specify the MMDB provider to use (default: maxmind)
    #[clap(long, value_name = "PROVIDER", default_value = "maxmind")]
    provider: String,

    /// Specify directory containing the MMDB database files
    #[clap(
        short = 'I',
        value_name = "DIR",
        value_hint = clap::ValueHint::DirPath,
        env = "GEOIP_MMDB_DIR"
    )]
    include: Option<Utf8PathBuf>,

    /// List available MMDB providers and their required files
    #[clap(long)]
    list_providers: bool,

    /// Display a list of available template substitution parameters to
    /// use in --template format string
    #[clap(short = 'L', long)]
    list_templates: bool,

    /// Input file(s) to process. Leave empty or use "-" to read from stdin
    #[clap(value_name = "FILE", value_hint = clap::ValueHint::FilePath)]
    input: Vec<Utf8PathBuf>,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum ArgsColorChoice {
    Always,
    Never,
    Auto,
}

fn main() -> ExitCode {
    // Use a separate run function to handle the actual work
    let err = match run_main() {
        Ok(code) => return code,
        Err(err) => err,
    };

    // Handle broken pipe errors gracefully
    if is_broken_pipe(&err) {
        return ExitCode::SUCCESS;
    }

    // Print detailed error information based on environment variables
    if std::env::var("RUST_BACKTRACE").is_ok_and(|v| v == "1")
        && std::env::var("RUST_LIB_BACKTRACE").map_or(true, |v| v == "1")
    {
        writeln!(&mut std::io::stderr(), "{:?}", err).unwrap();
    } else {
        writeln!(&mut std::io::stderr(), "{:#}", err).unwrap();
    }

    ExitCode::FAILURE
}

fn run_main() -> Result<ExitCode> {
    let mut args = Args::parse();

    // Create provider registry
    let mut provider_registry = mmdb::ProviderRegistry::default();

    // if user asks to list available providers
    if args.list_providers {
        let info = provider_registry.print_db_info()?;
        println!("{}", info);
        return Ok(ExitCode::SUCCESS);
    }

    // if user asks to see available template names
    if args.list_templates {
        // Set the active provider first
        provider_registry.set_active_provider(&args.provider)?;
        provider_registry.initialize_active_provider(args.include.clone())?;

        // Get and print available fields
        let fields = provider_registry.available_fields()?;
        println!(
            "Available template fields for provider '{}':",
            args.provider
        );
        for field in fields {
            println!(
                "{{{}}}\t{}\t(example: {})",
                field.name, field.description, field.example
            );
        }
        return Ok(ExitCode::SUCCESS);
    }

    // if no files specified, add stdin
    if args.input.is_empty() {
        args.input.push(Utf8PathBuf::from("-"));
    }

    // Check for legacy MAXMIND_MMDB_DIR environment variable if no include path is specified
    if args.include.is_none() {
        if let Ok(legacy_path) = std::env::var("MAXMIND_MMDB_DIR") {
            args.include = Some(Utf8PathBuf::from(legacy_path));
            eprintln!("Warning: MAXMIND_MMDB_DIR is deprecated, please use GEOIP_MMDB_DIR instead");
        }
    }

    // determine appropriate colormode. auto simply
    // tests if stdout is a tty (if so, then yes color)
    // or otherwise don't color if it's to a file or another pipe
    let colormode = match args.color {
        ArgsColorChoice::Auto => {
            if std::io::stdout().is_terminal() {
                ColorChoice::Always
            } else {
                ColorChoice::Never
            }
        }
        ArgsColorChoice::Always => ColorChoice::Always,
        ArgsColorChoice::Never => ColorChoice::Never,
    };

    // Process each input file
    run(args, colormode)?;

    Ok(ExitCode::SUCCESS)
}

#[inline(always)]
fn run(args: Args, colormode: ColorChoice) -> Result<()> {
    // Determine which IP types to include
    let include_private = args.all || !args.no_private;
    let include_loopback = args.all || !args.no_loopback;
    let include_broadcast = args.all || !args.no_broadcast;

    // Build the IP extractor with appropriate settings
    // New defaults include all IP types. Use ignore_*() to opt-out or .only_public() for convenience.
    let extractor = if !include_private && !include_loopback && !include_broadcast {
        // All filters active - use convenience method
        ExtractorBuilder::new().only_public().build()?
    } else {
        // Granular control with chaining
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
        builder.build()?
    };

    // Fast path: just extract IPs without MMDB lookups
    if args.justips {
        return run_justips(args, extractor);
    }

    // Initialize provider registry (only when needed)
    let mut provider_registry = mmdb::ProviderRegistry::default();
    provider_registry.set_active_provider(&args.provider)?;
    provider_registry.initialize_active_provider(args.include.clone())?;

    let geoipdb = geoip::GeoIPSed::new_with_provider(
        args.include.clone(),
        args.template.clone(),
        colormode,
        args.only_routable,
        provider_registry,
    )?;

    let mut out = io::BufWriter::with_capacity(65536, StandardStream::stdout(colormode));

    // Handle file-based tagging mode
    if args.tag_files {
        // Process each file as a whole rather than line by line
        files::tag_files(&args.input, &extractor, &mut out)?;
        out.flush()?;
        return Ok(());
    }

    // Use a larger initial capacity for cache to reduce rehashing
    let mut cache: HashMap<Vec<u8>, String> =
        HashMap::with_capacity_and_hasher(4096, Default::default());
    let only_matching = args.only_matching;
    let tag_mode = args.tag;
    let mut line_buffer = LineBufferBuilder::new().capacity(65536).build();

    for path in args.input {
        let file = FileOrStdin::from_path(path);
        let reader = file.reader()?;
        let mut lb_reader = LineBufferReader::new(reader, &mut line_buffer);

        while lb_reader.fill()? {
            let buffer = lb_reader.buffer();
            let lines = LineIter::new(b'\n', buffer);

            for line in lines {
                if only_matching {
                    for range in extractor.find_iter(line) {
                        let ip_bytes = &line[range.clone()];

                        if let Some(cached) = cache.get(ip_bytes) {
                            out.write_all(cached.as_bytes())?;
                            out.write_all(b"\n")?;
                        } else {
                            let ipstr = std::str::from_utf8(ip_bytes).unwrap_or("decode error");
                            if let Ok(ip) = ipstr.parse::<std::net::IpAddr>() {
                                let result = geoipdb.lookup(ip, ipstr);
                                out.write_all(result.as_bytes())?;
                                out.write_all(b"\n")?;
                                if cache.len() < 100000 {
                                    cache.insert(ip_bytes.to_vec(), result);
                                }
                            } else {
                                out.write_all(ipstr.as_bytes())?;
                                out.write_all(b"\n")?;
                            }
                        }
                    }
                } else if tag_mode {
                    let mut tagged = Tagged::new(line);
                    for range in extractor.find_iter(line) {
                        let ipstr =
                            std::str::from_utf8(&line[range.clone()]).unwrap_or("decode error");
                        tagged = tagged.tag(
                            Tag::new(ipstr.to_owned())
                                .with_range(range)
                                .with_decoration(String::new()),
                        );
                    }
                    tagged.write_json(&mut out)?;
                } else {
                    let mut last_pos = 0;
                    for range in extractor.find_iter(line) {
                        out.write_all(&line[last_pos..range.start])?;
                        let ip_bytes = &line[range.clone()];

                        if let Some(cached) = cache.get(ip_bytes) {
                            out.write_all(cached.as_bytes())?;
                        } else {
                            let ipstr = std::str::from_utf8(ip_bytes).unwrap_or("decode error");
                            if let Ok(ip) = ipstr.parse::<std::net::IpAddr>() {
                                let result = geoipdb.lookup(ip, ipstr);
                                out.write_all(result.as_bytes())?;
                                if cache.len() < 100000 {
                                    cache.insert(ip_bytes.to_vec(), result);
                                }
                            } else {
                                out.write_all(ipstr.as_bytes())?;
                            }
                        }
                        last_pos = range.end;
                    }
                    out.write_all(&line[last_pos..])?;
                }
            }
            lb_reader.consume_all();
        }
        out.flush()?;
    }

    Ok(())
}

/// Fast path for extracting IPs without MMDB lookups or templating
/// Always outputs just IPs, one per line
#[inline(always)]
fn run_justips(args: Args, extractor: geoipsed::Extractor) -> Result<()> {
    let mut out = io::BufWriter::with_capacity(65536, io::stdout());
    let mut line_buffer = LineBufferBuilder::new().capacity(65536).build();

    for path in args.input {
        let file = FileOrStdin::from_path(path);
        let reader = file.reader()?;
        let mut lb_reader = LineBufferReader::new(reader, &mut line_buffer);

        while lb_reader.fill()? {
            let buffer = lb_reader.buffer();
            let lines = LineIter::new(b'\n', buffer);

            for line in lines {
                // Always output just IPs, one per line
                for range in extractor.find_iter(line) {
                    out.write_all(&line[range])?;
                    out.write_all(b"\n")?;
                }
            }
            lb_reader.consume_all();
        }
        out.flush()?;
    }

    Ok(())
}
