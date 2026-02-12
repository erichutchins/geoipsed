use ip_extract::ExtractorBuilder;

fn main() {
    let extractor = ExtractorBuilder::new().build().unwrap();

    let input = b"Link-local with scope: fe80::1%eth0 and fe80::dead:beef%en0";

    let results: Vec<String> = extractor
        .find_iter(input)
        .map(|r| String::from_utf8_lossy(&input[r]).to_string())
        .collect();

    println!("Input: {}", String::from_utf8_lossy(input));
    println!("Results: {:?}", results);
}
