use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::input::FileOrStdin;
use crate::{Tag, Tagged, TextData, Extractor};

/// Helper function to tag content and write as JSON.
///
/// This function extracts all IP addresses from the content and outputs them as JSON tags.
fn tag_content(content: &[u8], extractor: &Extractor, output: &mut dyn Write) -> Result<()> {
    // Create a tagged object for the content
    let mut tagged = Tagged::new(content);

    // Find all IP addresses in the content
    for range in extractor.find_iter(content) {
        let ip_slice = &content[range.clone()];
        let ip_str = std::str::from_utf8(ip_slice)
            .context("Invalid UTF-8 in IP address")?
            .to_string();

        // Add the tag with its range
        tagged = tagged.tag(Tag::new(ip_str).with_range(range));
    }

    // Only output if we found matches
    if !tagged.tags().is_empty() {
        // Set the text data explicitly - use lossy conversion for non-UTF8 text
        let text_str = String::from_utf8_lossy(content).to_string();
        let mut tagged = tagged;
        tagged.set_text_data(TextData { text: text_str });

        // Write as JSON
        tagged.write_json(output)?;
        writeln!(output)?;
    }

    Ok(())
}

/// Process a file and extract IP addresses as tags.
///
/// This function reads the entire file content and extracts all IP addresses,
/// outputting the tags as JSON.
pub fn tag_file(path: &Path, extractor: &Extractor, output: &mut dyn Write) -> Result<()> {
    let mut content = Vec::new();
    let mut file =
        File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    file.read_to_end(&mut content)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    tag_content(&content, extractor, output)
}

/// Process multiple files or stdin, extracting IP addresses as tags.
///
/// This function iterates over each input path and processes the file content,
/// outputting the tags as JSON.
pub fn tag_files(
    paths: &[Utf8PathBuf],
    extractor: &Extractor,
    output: &mut dyn Write,
) -> Result<()> {
    for path in paths {
        let input = FileOrStdin::from_path(path.clone());

        match input {
            FileOrStdin::File(path) => {
                let path = path.as_std_path();
                tag_file(path, extractor, output)?;
            }
            FileOrStdin::Stdin => {
                let mut content = Vec::new();
                io::stdin()
                    .read_to_end(&mut content)
                    .context("Failed to read from stdin")?;

                tag_content(&content, extractor, output)?;
            }
        }
    }

    Ok(())
}
