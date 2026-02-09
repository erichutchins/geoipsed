use anyhow::Result;
use ip_extract::{ExtractorBuilder, Tag, Tagged};
use std::io::stdout;

fn main() -> Result<()> {
    // 1. Configure and build the extractor.
    // We'll include IPv4 and private IPs for this example.
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .private_ips(true)
        .build()?;

    // 2. Sample input bytes (e.g., from a log file).
    let input = b"Traffic from 10.0.0.5 to 8.8.8.8 and also 2001:4860:4860::8888";

    // 3. Create a Tagged container to hold our matches and original text.
    let mut tagged = Tagged::new(input);

    // 4. Find all IP addresses.
    println!("Scanning text...\n");
    for range in extractor.find_iter(input) {
        let ip_bytes = &input[range.clone()];
        let ip_str = std::str::from_utf8(ip_bytes)?;
        
        println!("  Found: {} at {:?}", ip_str, range);

        // Add a tag to our container. 
        // We can also add "decoration" (simulating a DB lookup).
        let decoration = format!("[{}]", ip_str); // Simple decoration
        tagged = tagged.tag(
            Tag::new(ip_str)
                .with_range(range)
                .with_decoration(decoration)
        );
    }

    // 5. Output the results.
    println!("\n--- Decorated Output ---");
    tagged.write(&mut stdout())?;
    println!("\n");

    println!("--- JSON Output ---");
    tagged.write_json(&mut stdout())?;
    println!();

    Ok(())
}
