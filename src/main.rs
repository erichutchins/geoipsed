use anyhow::{Error, Result};
use camino::Utf8PathBuf;
use clap::{Parser, ValueEnum};
use grep_cli::{self, stdout};
use regex::bytes::Regex;
use ripline::{
    line_buffer::{LineBufferBuilder, LineBufferReader},
    lines::LineIter,
    LineTerminator,
};
use rustc_hash::FxHashMap as HashMap;
use std::fs::File;
use std::io::{self, BufReader, IsTerminal, Read, Write};
use std::process::exit;
use termcolor::ColorChoice;

pub mod geoip;

const BUFFERSIZE: usize = 64 * 1024;

// via https://github.com/sstadick/hck/blob/master/src/main.rs#L90
/// Check if err is a broken pipe.
#[inline]
fn is_broken_pipe(err: &Error) -> bool {
    if let Some(io_err) = err.root_cause().downcast_ref::<io::Error>() {
        if io_err.kind() == io::ErrorKind::BrokenPipe {
            return true;
        }
    }
    false
}

// via https://github.com/sstadick/crabz/blob/main/src/main.rs#L82
/// Get a buffered input reader from stdin or a file
fn get_input(path: Option<Utf8PathBuf>) -> Result<Box<dyn Read + Send + 'static>> {
    let reader: Box<dyn Read + Send + 'static> = match path {
        Some(path) => {
            if path.as_os_str() == "-" {
                Box::new(BufReader::with_capacity(BUFFERSIZE, io::stdin()))
            } else {
                Box::new(BufReader::with_capacity(BUFFERSIZE, File::open(path)?))
            }
        }
        None => Box::new(BufReader::with_capacity(BUFFERSIZE, io::stdin())),
    };
    Ok(reader)
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

    /// Specify directory containing GeoLite2-ASN.mmdb and GeoLite2-City.mmdb
    #[clap(short = 'I', value_name = "DIR", value_hint = clap::ValueHint::DirPath, env = "MAXMIND_MMDB_DIR")]
    include: Option<Utf8PathBuf>,

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

fn main() -> Result<()> {
    let mut args = Args::parse();

    // if user asks to see available template names
    if args.list_templates {
        geoip::print_ip_field_names();
        return Ok(());
    }

    // if no files specified, add stdin
    if args.input.is_empty() {
        args.input.push(Utf8PathBuf::from("-"));
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

    // invoke the command!
    let invoke = if args.only_matching {
        run_onlymatching(args, colormode)
    } else {
        run(args, colormode)
    };

    match invoke {
        Err(e) if is_broken_pipe(&e) => exit(0),
        other => other,
    }
}

#[inline]
fn run(args: Args, colormode: ColorChoice) -> Result<()> {
    let geoipdb = geoip::GeoIPSed::new(args.include, args.template, colormode);
    let re = Regex::new(geoip::REGEX_PATTERN).unwrap();
    let mut out = stdout(colormode);
    let mut cache: HashMap<String, String> = HashMap::default();

    for path in args.input {
        let reader = get_input(Some(path))?;
        let terminator = LineTerminator::byte(b'\n');
        let mut line_buffer = LineBufferBuilder::new().build();
        let mut lb_reader = LineBufferReader::new(reader, &mut line_buffer);
        let mut _lastpos: usize = 0;

        // line reader
        while lb_reader.fill()? {
            let lines = LineIter::new(terminator.as_byte(), lb_reader.buffer());
            for line in lines {
                _lastpos = 0;
                for m in re.find_iter(line) {
                    let ipstr = String::from_utf8(m.as_bytes().to_vec())
                        .unwrap_or_else(|_| "decode error".into());
                    // lookup ip in cache or decorate if new
                    let decorated: &str = cache
                        .entry(ipstr)
                        .or_insert_with_key(|key| geoipdb.lookup(key));

                    // print gap from last match to current match
                    out.write_all(&line[_lastpos..m.start()])?;
                    // print decorated ip
                    out.write_all(decorated.as_bytes())?;
                    _lastpos = m.end();
                }
                // add trailing...(or entire line in case of no matches)
                out.write_all(&line[_lastpos..])?;
            }
            lb_reader.consume_all();
        }
        out.flush()?;
    }
    Ok(())
}

#[inline]
fn run_onlymatching(args: Args, colormode: ColorChoice) -> Result<()> {
    let geoipdb = geoip::GeoIPSed::new(args.include, args.template, colormode);
    let re = Regex::new(geoip::REGEX_PATTERN).unwrap();
    let mut out = stdout(colormode);
    let mut cache: HashMap<String, String> = HashMap::default();

    for path in args.input {
        let reader = get_input(Some(path))?;
        let terminator = LineTerminator::byte(b'\n');
        let mut line_buffer = LineBufferBuilder::new().build();
        let mut lb_reader = LineBufferReader::new(reader, &mut line_buffer);

        // line reader
        while lb_reader.fill()? {
            let lines = LineIter::new(terminator.as_byte(), lb_reader.buffer());
            for line in lines {
                for m in re.find_iter(line) {
                    let ipstr = String::from_utf8(m.as_bytes().to_vec())
                        .unwrap_or_else(|_| "decode error".into());
                    // lookup ip in cache or decorate if new
                    let decorated: &str = cache
                        .entry(ipstr)
                        .or_insert_with_key(|key| geoipdb.lookup(key));

                    // *only* print decorated ip
                    out.write_all(decorated.as_bytes())?;
                    // and a newline
                    out.write_all(&[b'\n'])?;
                }
            }
            lb_reader.consume_all();
        }
        out.flush()?;
    }
    Ok(())
}
