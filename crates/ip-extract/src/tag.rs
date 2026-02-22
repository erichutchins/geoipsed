use serde::Serialize;
use std::io::{self, Write};
use std::ops::Range;

/// A tag representing an IP address found in text.
#[derive(Clone, Debug, Serialize)]
pub struct Tag {
    /// The IP address text itself.
    #[serde(rename = "value")]
    ip: String,
    /// The range in the original text where the IP was found.
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<Range<usize>>,
    /// The decorated IP with geolocation information.
    #[serde(skip_serializing_if = "Option::is_none")]
    decorated: Option<String>,
}

impl Tag {
    /// Create a new tag for an IP address.
    ///
    /// The `ip` should be the literal text of the IP address as found in the input.
    #[inline]
    pub fn new<S: Into<String>>(ip: S) -> Tag {
        Tag {
            ip: ip.into(),
            range: None,
            decorated: None,
        }
    }

    /// Set the byte range [start, end) where this tag was found in the original text.
    #[inline]
    #[must_use]
    pub fn with_range(mut self, range: Range<usize>) -> Self {
        self.range = Some(range);
        self
    }

    /// Set a "decorated" version of the IP (e.g., with geolocation metadata).
    ///
    /// This string will be used instead of the original IP when calling `Tagged::write`.
    #[inline]
    pub fn with_decoration<S: Into<String>>(mut self, decorated: S) -> Self {
        self.decorated = Some(decorated.into());
        self
    }

    /// Get the IP address text.
    #[inline]
    #[must_use]
    pub fn ip(&self) -> &str {
        &self.ip
    }

    /// Get the range of this tag in the original text, if available.
    #[inline]
    #[must_use]
    pub fn range(&self) -> Option<&Range<usize>> {
        self.range.as_ref()
    }

    /// Get the decorated version of this IP, if available.
    #[inline]
    #[must_use]
    pub fn decorated(&self) -> Option<&str> {
        self.decorated.as_deref()
    }
}

/// A line of text with tags.
#[derive(Clone, Debug, Serialize)]
pub struct Tagged {
    /// The original text.
    #[serde(skip_serializing)]
    text: Vec<u8>,
    /// The tags found in the text.
    tags: Vec<Tag>,
    /// The original text as a string (for JSON serialization).
    #[serde(rename = "data")]
    text_data: Option<TextData>,
}

/// Represents the text data for JSON serialization.
#[derive(Clone, Debug, Serialize)]
pub struct TextData {
    /// The original text as a string.
    pub text: String,
}

impl Tagged {
    /// Create a new `Tagged` container for a slice of text.
    ///
    /// This container holds the original text and will collect any `Tag`s found within it.
    #[inline]
    #[must_use]
    pub fn new(text: &[u8]) -> Tagged {
        // Pre-allocate a reasonable capacity for tags based on text length
        let capacity = if text.len() > 1000 { 16 } else { 4 };
        Tagged {
            text: text.to_vec(),
            tags: Vec::with_capacity(capacity), // Most lines have few IPs
            text_data: None,
        }
    }

    /// Adds a tag to this text.
    ///
    /// The tag should contain a range that corresponds to its position in `self.text()`.
    #[inline]
    #[must_use]
    pub fn tag(mut self, tag: Tag) -> Self {
        self.tags.push(tag);
        self
    }

    /// Get the tags in this text.
    #[inline]
    #[must_use]
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    /// Get the original text.
    #[inline]
    #[must_use]
    pub fn text(&self) -> &[u8] {
        &self.text
    }

    /// Explicitly sets the text data used for JSON serialization.
    #[inline]
    pub fn set_text_data(&mut self, data: TextData) {
        self.text_data = Some(data);
    }

    /// Writes the text to the given writer, replacing tagged IPs with their decorated versions.
    ///
    /// If a tag has a `decorated()` value, that value is written instead of the original
    /// bytes in its `range()`. If no decoration is present, the original bytes are written.
    ///
    /// Tags MUST be sorted by their start position for this to work correctly.
    #[inline]
    pub fn write<W: Write>(&self, wtr: &mut W) -> io::Result<()> {
        // Fast path for no tags
        if self.tags.is_empty() {
            return wtr.write_all(&self.text);
        }

        // If we have only one tag (common case), optimize for it
        if self.tags.len() == 1 {
            let tag = &self.tags[0];
            if let Some(range) = tag.range() {
                // Write the text before the tag
                wtr.write_all(&self.text[..range.start])?;

                // Write the decorated version if available, or the original IP
                if let Some(decorated) = tag.decorated() {
                    wtr.write_all(decorated.as_bytes())?;
                } else {
                    wtr.write_all(&self.text[range.clone()])?;
                }

                // Write the text after the tag
                wtr.write_all(&self.text[range.end..])?;
                return Ok(());
            }
        }

        // If we have 2 tags (another common case), optimize for it
        if self.tags.len() == 2 {
            // Get the two tags
            let mut tag1 = &self.tags[0];
            let mut tag2 = &self.tags[1];

            // Ensure tag1 comes before tag2
            if let (Some(range1), Some(range2)) = (tag1.range(), tag2.range()) {
                if range1.start > range2.start {
                    std::mem::swap(&mut tag1, &mut tag2);
                }

                // Write in three parts: before tag1, tag1, between tags, tag2, after tag2
                wtr.write_all(&self.text[..range1.start])?;

                if let Some(decorated) = tag1.decorated() {
                    wtr.write_all(decorated.as_bytes())?;
                } else {
                    wtr.write_all(&self.text[range1.clone()])?;
                }

                wtr.write_all(&self.text[range1.end..range2.start])?;

                if let Some(decorated) = tag2.decorated() {
                    wtr.write_all(decorated.as_bytes())?;
                } else {
                    wtr.write_all(&self.text[range2.clone()])?;
                }

                wtr.write_all(&self.text[range2.end..])?;
                return Ok(());
            }
        }

        // For multiple tags, process them in order
        // Tags should always be sorted by position since the extractor finds matches left-to-right
        #[cfg(debug_assertions)]
        {
            for i in 1..self.tags.len() {
                if let (Some(prev), Some(curr)) = (self.tags[i - 1].range(), self.tags[i].range()) {
                    debug_assert!(prev.start <= curr.start, "Tags must be sorted by position");
                }
            }
        }

        let mut last_end = 0;
        for tag in &self.tags {
            if let Some(range) = tag.range() {
                // Write the text between the previous tag and this one
                wtr.write_all(&self.text[last_end..range.start])?;

                // Write the decorated version if available, or the original IP
                if let Some(decorated) = tag.decorated() {
                    wtr.write_all(decorated.as_bytes())?;
                } else {
                    wtr.write_all(&self.text[range.clone()])?;
                }

                last_end = range.end;
            }
        }

        // Write any remaining text
        if last_end < self.text.len() {
            wtr.write_all(&self.text[last_end..])?;
        }

        Ok(())
    }

    /// Writes the `Tagged` object as a JSON object to the given writer.
    ///
    /// This is useful for exporting structured metadata about the IPs found in the text.
    #[inline]
    pub fn write_json<W: Write + ?Sized>(&mut self, wtr: &mut W) -> io::Result<()> {
        // Set the text data for JSON serialization
        if self.text_data.is_none() {
            // Fast path for direct UTF-8 conversion
            if self.text.is_empty() {
                self.text_data = Some(TextData {
                    text: String::new(),
                });
            } else if let Ok(s) = std::str::from_utf8(&self.text) {
                // Direct conversion without allocation
                self.text_data = Some(TextData {
                    text: s.to_string(),
                });
            } else {
                // Fallback for non-UTF8
                self.text_data = Some(TextData {
                    text: String::from_utf8_lossy(&self.text).to_string(),
                });
            }
        }

        // Serialize to JSON using the faster non-pretty writer
        serde_json::to_writer(wtr, self)?;
        Ok(())
    }
}
