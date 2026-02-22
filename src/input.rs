use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

/// A source that can be either a file or stdin.
#[derive(Default, Clone, Debug)]
pub enum FileOrStdin {
    /// Input from a file.
    File(Utf8PathBuf),
    /// Input from stdin.
    #[default]
    Stdin,
}

impl fmt::Display for FileOrStdin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FileOrStdin::File(path) => write!(f, "{path}"),
            FileOrStdin::Stdin => write!(f, "<stdin>"),
        }
    }
}

impl FileOrStdin {
    /// Create a new `FileOrStdin` from a path.
    ///
    /// If the path is "-", stdin is used.
    #[must_use]
    pub fn from_path(path: Utf8PathBuf) -> Self {
        if path.as_str() == "-" {
            FileOrStdin::Stdin
        } else {
            FileOrStdin::File(path)
        }
    }

    /// Return a display string for the input source.
    #[must_use]
    pub fn display(&self) -> String {
        match self {
            FileOrStdin::File(path) => path.to_string(),
            FileOrStdin::Stdin => "<stdin>".to_string(),
        }
    }

    /// Open the input source as a reader.
    pub fn reader(&self) -> Result<InputReader> {
        match self {
            FileOrStdin::File(path) => {
                let file =
                    File::open(path).with_context(|| format!("failed to open file: {path}"))?;
                Ok(InputReader::File(BufReader::new(file)))
            }
            FileOrStdin::Stdin => Ok(InputReader::Stdin(BufReader::new(io::stdin()))),
        }
    }
}

/// A reader for input from either a file or stdin.
pub enum InputReader {
    /// A reader for a file.
    File(BufReader<File>),
    /// A reader for stdin.
    Stdin(BufReader<io::Stdin>),
}

impl BufRead for InputReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        match self {
            InputReader::File(ref mut rdr) => rdr.fill_buf(),
            InputReader::Stdin(ref mut rdr) => rdr.fill_buf(),
        }
    }

    fn consume(&mut self, amt: usize) {
        match self {
            InputReader::File(ref mut rdr) => rdr.consume(amt),
            InputReader::Stdin(ref mut rdr) => rdr.consume(amt),
        }
    }
}

impl Read for InputReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            InputReader::File(ref mut rdr) => rdr.read(buf),
            InputReader::Stdin(ref mut rdr) => rdr.read(buf),
        }
    }
}

impl InputReader {
    /// Read the entire input into a string.
    pub fn read_to_string(&mut self) -> Result<String> {
        let mut buf = String::new();
        Read::read_to_string(self, &mut buf).context("failed to read input")?;
        Ok(buf)
    }
}
