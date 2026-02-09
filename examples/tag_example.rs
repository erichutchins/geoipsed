use anyhow::Result;
use geoipsed::{ExtractorBuilder, Tag, Tagged, TextData};
use std::io::stdout;

fn main() -> Result<()> {
    // Build an IP address extractor
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .private_ips(true)
        .build()?;

    // Sample text with IP addresses
    let text = "Connection from 81.2.69.205 to 175.16.199.37 and IPv6 2001:480::52 detected";

    // Create a tagged object with our text
    let mut tagged = Tagged::new(text.as_bytes());

    // Find and tag all IP addresses
    for range in extractor.find_iter(text.as_bytes()) {
        let ip_slice = &text.as_bytes()[range.clone()];
        let ip_str = String::from_utf8_lossy(ip_slice).to_string();
        tagged = tagged.tag(Tag::new(ip_str).with_range(range));
    }

    // Set the text data for JSON output
    tagged.set_text_data(TextData {
        text: text.to_string(),
    });

    // Write the tagged data as JSON
    let mut output = stdout();
    tagged.write_json(&mut output)?;
    println!(); // Add a newline

    Ok(())
}
