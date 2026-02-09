use anyhow::{Error, Result};
use bstr::io::BufReadExt;
use camino::Utf8PathBuf;
use clap::{Parser, ValueEnum};
use rustc_hash::FxHashMap as HashMap;
use std::io::{self, BufRead, IsTerminal, Write};
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

    // Initialize provider registry
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

    // Build the IP extractor with appropriate settings
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .private_ips(include_private)
        .loopback_ips(include_loopback)
        .broadcast_ips(include_broadcast)
        .only_routable(args.only_routable)
        .build()?;

    let mut out = StandardStream::stdout(colormode);

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

    for path in args.input {
        let input = FileOrStdin::from_path(path);
        let mut reader = input.reader()?;

        // In only_matching mode, skip line iteration entirely and run regex
        // directly over the reader's buffer.
        if only_matching {
            loop {
                let buf = reader.fill_buf()?;
                if buf.is_empty() {
                    break;
                }
                let len = buf.len();

                for range in extractor.find_iter(buf) {
                    let ip_bytes = &buf[range.clone()];

                    if let Some(cached) = cache.get(ip_bytes) {
                        out.write_all(cached.as_bytes())?;
                        out.write_all(b"\n")?;
                    } else {
                        // Only perform UTF-8 validation on cache miss
                        let ipstr = std::str::from_utf8(ip_bytes).unwrap_or("decode error");
                        let result = geoipdb.lookup(ipstr);
                        out.write_all(result.as_bytes())?;
                        out.write_all(b"\n")?;
                        cache.insert(ip_bytes.to_vec(), result);
                    }
                }
                reader.consume(len);
            }
        } else {
            // Process line-by-line for normal and tag modes
            reader.for_byte_line_with_terminator(|line| {
                if tag_mode {
                    let mut tagged = Tagged::new(line);

                    for range in extractor.find_iter(line) {
                        let ipstr =
                            std::str::from_utf8(&line[range.clone()]).unwrap_or("decode error");
                        // In tag mode, we don't decorate, just tag.
                        tagged = tagged.tag(
                            Tag::new(ipstr.to_owned())
                                .with_range(range)
                                .with_decoration(String::new()),
                        );
                    }
                    tagged.write_json(&mut out)?;
                } else {
                    // Fast path: Stream directly to avoid allocations for Tagged/Tag structs
                    let mut last_pos = 0;
                    for range in extractor.find_iter(line) {
                        // Write text before the match
                        out.write_all(&line[last_pos..range.start])?;

                        let ip_bytes = &line[range.clone()];

                        if let Some(cached) = cache.get(ip_bytes) {
                            out.write_all(cached.as_bytes())?;
                        } else {
                            // Only perform UTF-8 validation on cache miss
                            let ipstr = std::str::from_utf8(ip_bytes).unwrap_or("decode error");
                            let result = geoipdb.lookup(ipstr);
                            out.write_all(result.as_bytes())?;
                            cache.insert(ip_bytes.to_vec(), result);
                        }
                        last_pos = range.end;
                    }
                    // Write remaining text
                    out.write_all(&line[last_pos..])?;
                }

                Ok(true)
            })?;
        }
        out.flush()?;
    }

    Ok(())
}
