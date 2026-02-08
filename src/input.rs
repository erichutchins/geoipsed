use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::fmt;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};

/// Represents a line of text read from input.
pub struct Line<'a> {
    /// The full line, including the line terminator.
    full: &'a [u8],
    /// The content of the line, excluding the line terminator.
    content: &'a [u8],
}

impl<'a> Line<'a> {
    /// Create a new Line from a byte slice, expected to be a complete line.
    #[inline]
    pub fn new(full: &'a [u8]) -> Line<'a> {
        let content = if full.last() == Some(&b'\n') {
            &full[..full.len() - 1]
        } else {
            full
        };
        Line { full, content }
    }

    /// Get the full line, including the line terminator if present.
    #[inline]
    pub fn full(&self) -> &'a [u8] {
        self.full
    }

    /// Get the content of the line, excluding the line terminator.
    #[inline]
    pub fn content(&self) -> &'a [u8] {
        self.content
    }
}

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
            FileOrStdin::File(path) => write!(f, "{}", path),
            FileOrStdin::Stdin => write!(f, "<stdin>"),
        }
    }
}

impl FileOrStdin {
    /// Create a new FileOrStdin from a path.
    ///
    /// If the path is "-", stdin is used.
    pub fn from_path(path: Utf8PathBuf) -> Self {
        if path.as_str() == "-" {
            FileOrStdin::Stdin
        } else {
            FileOrStdin::File(path)
        }
    }

    /// Return a display string for the input source.
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
                    File::open(path).with_context(|| format!("failed to open file: {}", path))?;
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
    /// Process each byte line from the input.
    ///
    /// The provided function is called for each line. If it returns `Ok(true)`,
    /// processing continues. If it returns `Ok(false)`, processing stops.
    /// If it returns an error, processing stops and the error is returned.
    pub fn for_byte_line<F>(&mut self, mut f: F) -> Result<()>
    where
        F: FnMut(Line<'_>) -> Result<bool>,
    {
        let mut buf = Vec::with_capacity(1024);
        loop {
            buf.clear();
            let n = self
                .read_until(b'\n', &mut buf)
                .context("failed to read line")?;
            if n == 0 {
                break;
            }
            let line = Line::new(&buf);
            if !f(line)? {
                break;
            }
        }
        Ok(())
    }

    /// Read the entire input into a string.
    pub fn read_to_string(&mut self) -> Result<String> {
        let mut buf = String::new();
        Read::read_to_string(self, &mut buf).context("failed to read input")?;
        Ok(buf)
    }
}
