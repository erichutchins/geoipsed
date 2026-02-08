use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::extractor::Extractor;
use crate::input::FileOrStdin;
use crate::tag::{Tag, Tagged, TextData};

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

    // Create a tagged object for the whole file
    let mut tagged = Tagged::new(&content);

    // Find all IP addresses in the file
    for range in extractor.find_iter(&content) {
        let ip_slice = &content[range.clone()];
        let ip_str = String::from_utf8_lossy(ip_slice).to_string();

        // Add the tag with its range
        tagged = tagged.tag(Tag::new(ip_str).with_range(range));
    }

    // Only output if we found matches
    if !tagged.tags().is_empty() {
        // Set the text data explicitly
        let text_str = String::from_utf8_lossy(&content).to_string();
        let mut tagged = tagged;
        tagged.set_text_data(TextData { text: text_str });

        // Write as JSON
        tagged.write_json(output)?;
        writeln!(output)?;
    }

    Ok(())
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

                // Create a tagged object for the whole stdin content
                let mut tagged = Tagged::new(&content);

                // Find all IP addresses in the content
                for range in extractor.find_iter(&content) {
                    let ip_slice = &content[range.clone()];
                    let ip_str = String::from_utf8_lossy(ip_slice).to_string();

                    // Add the tag with its range
                    tagged = tagged.tag(Tag::new(ip_str).with_range(range));
                }

                // Only output if we found matches
                if !tagged.tags().is_empty() {
                    // Set the text data explicitly
                    let text_str = String::from_utf8_lossy(&content).to_string();
                    let mut tagged = tagged;
                    tagged.set_text_data(TextData { text: text_str });

                    // Write as JSON
                    tagged.write_json(output)?;
                    writeln!(output)?;
                }
            }
        }
    }

    Ok(())
}
