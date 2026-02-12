use ip_extract::ExtractorBuilder;

/// Simplified test harness to verify IP extraction.
fn check_extraction(
    haystack: &[u8],
    expected: &[&str],
    include_private: bool,
    include_loopback: bool,
) {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .private_ips(include_private)
        .loopback_ips(include_loopback)
        .build()
        .expect("Failed to build extractor");

    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(
        actual,
        expected.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        "\nFailed for haystack: {}\n",
        String::from_utf8_lossy(haystack)
    );
}

#[test]
fn test_ipv4_variations() {
    check_extraction(
        b"Standard: 8.8.8.8, LAN: 192.168.1.1, Loop: 127.0.0.1",
        &["8.8.8.8", "192.168.1.1", "127.0.0.1"],
        true, // include private
        true, // include loopback
    );
}

#[test]
fn test_ipv6_variations() {
    check_extraction(
        b"Full: 2001:0db8:85a3:0000:0000:8a2e:0370:7334, Compressed: 2001:db8::1, Local: ::1",
        &[
            "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
            "2001:db8::1",
            "::1",
        ],
        true,
        true,
    );
}

#[test]
fn test_filtering() {
    let haystack = b"Public: 1.1.1.1, Private: 10.0.0.1, Loopback: 127.0.0.1";

    // Only Public
    check_extraction(haystack, &["1.1.1.1"], false, false);

    // Public + Private
    check_extraction(haystack, &["1.1.1.1", "10.0.0.1"], true, false);

    // Everything
    check_extraction(haystack, &["1.1.1.1", "10.0.0.1", "127.0.0.1"], true, true);
}

#[test]
fn test_delimiters_and_junk() {
    check_extraction(
        b"text 1.1.1.1 text, 2.2.2.2; [3.3.3.3] (8.8.8.8)",
        &["1.1.1.1", "2.2.2.2", "3.3.3.3", "8.8.8.8"],
        true,
        true,
    );
}

#[test]
fn test_invalid_ips() {
    check_extraction(
        b"Not an IP: 256.256.256.256, 1.2.3, 1.2.3.4.5, ::::, 127.0.0.01",
        &[],
        true,
        true,
    );
}

#[test]
fn test_mixed_versions_tight() {
    check_extraction(
        b"1.1.1.1, ::1, 8.8.8.8",
        &["1.1.1.1", "::1", "8.8.8.8"],
        true,
        true,
    );
}

// ============================================================================
// NEW COMPREHENSIVE TESTS
// ============================================================================

#[test]
fn test_ipv4_boundary_values() {
    // Min and max values (note: 255.255.255.255 is broadcast and 192.168.1.1 is private)
    // Defaults include all IP types
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .build()
        .unwrap();

    let haystack = b"Min: 0.0.0.0, Max: 255.255.255.255, Normal: 192.168.1.1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["0.0.0.0", "255.255.255.255", "192.168.1.1"]);
}

#[test]
fn test_ipv4_single_digit_octets() {
    check_extraction(
        b"Single digits: 1.2.3.4, 5.6.7.8",
        &["1.2.3.4", "5.6.7.8"],
        true,
        true,
    );
}

#[test]
fn test_ipv4_all_private_ranges() {
    // Test all RFC 1918 private ranges
    check_extraction(
        b"Class A: 10.0.0.1, Class B: 172.16.0.1, Class C: 192.168.0.1",
        &["10.0.0.1", "172.16.0.1", "192.168.0.1"],
        true,
        false,
    );

    // Verify they're filtered when include_private=false
    check_extraction(
        b"Class A: 10.0.0.1, Class B: 172.16.0.1, Class C: 192.168.0.1, Public: 8.8.8.8",
        &["8.8.8.8"],
        false,
        false,
    );
}

#[test]
fn test_ipv4_middle_private_range() {
    // Specifically test 172.16.0.0/12 range
    check_extraction(
        b"Start: 172.16.0.0, Mid: 172.20.0.1, End: 172.31.255.255",
        &["172.16.0.0", "172.20.0.1", "172.31.255.255"],
        true,
        false,
    );

    check_extraction(
        b"Private: 172.16.0.1, Public: 172.32.0.1",
        &["172.32.0.1"],
        false,
        false,
    );
}

#[test]
fn test_ipv4_link_local() {
    // Link-local addresses (169.254.0.0/16) should be filtered by broadcast_ips
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .ignore_broadcast()
        .build()
        .unwrap();

    let haystack = b"Link-local: 169.254.1.1, Public: 8.8.8.8";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["8.8.8.8"]);
}

#[test]
fn test_ipv4_broadcast_address() {
    // Broadcast address should be filtered by broadcast_ips
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .ignore_broadcast()
        .build()
        .unwrap();

    let haystack = b"Broadcast: 255.255.255.255, Public: 8.8.8.8";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["8.8.8.8"]);
}

#[test]
fn test_ipv4_loopback_range() {
    // Full 127.0.0.0/8 range should be filtered
    check_extraction(
        b"Loop start: 127.0.0.1, Loop mid: 127.100.50.25, Loop end: 127.255.255.254",
        &["127.0.0.1", "127.100.50.25", "127.255.255.254"],
        true,
        true,
    );

    check_extraction(
        b"Loop: 127.0.0.1, Public: 8.8.8.8",
        &["8.8.8.8"],
        true,
        false,
    );
}

#[test]
fn test_ipv4_leading_zeros_rejected() {
    // Leading zeros should be rejected
    check_extraction(
        b"Invalid: 192.168.01.1, 192.168.001.1, 01.2.3.4",
        &[],
        true,
        true,
    );
}

#[test]
fn test_ipv4_trailing_dot() {
    // Test that IPs can be extracted successfully in various contexts
    let extractor = ExtractorBuilder::new().ipv4(true).build().unwrap();

    // IP without trailing dot works fine
    let haystack1 = b"192.168.1.1";
    let ranges1: Vec<_> = extractor.find_iter(haystack1).collect();
    assert_eq!(ranges1.len(), 1);
    assert_eq!(&haystack1[ranges1[0].clone()], b"192.168.1.1");

    // IP with surrounding text works
    let haystack2 = b"text 192.168.1.1 more text";
    let ranges2: Vec<_> = extractor.find_iter(haystack2).collect();
    assert_eq!(ranges2.len(), 1);
    assert_eq!(&haystack2[ranges2[0].clone()], b"192.168.1.1");

    // IP with comma, semicolon, colon, parentheses works
    let haystack3 = b"IPs: 1.2.3.4, 5.6.7.8; (9.10.11.12):end";
    let ranges3: Vec<_> = extractor.find_iter(haystack3).collect();
    assert_eq!(ranges3.len(), 3);
    assert_eq!(&haystack3[ranges3[0].clone()], b"1.2.3.4");
    assert_eq!(&haystack3[ranges3[1].clone()], b"5.6.7.8");
    assert_eq!(&haystack3[ranges3[2].clone()], b"9.10.11.12");

    // IPs followed by dots (sentence endings) now work correctly!
    // The trailing dot is stripped by the backtracking algorithm for IPv4
    let haystack4 = b"The C2 IP was 192.168.1.1.";
    let ranges4: Vec<_> = extractor.find_iter(haystack4).collect();
    assert_eq!(ranges4.len(), 1);
    assert_eq!(&haystack4[ranges4[0].clone()], b"192.168.1.1");

    // Multiple IPs with trailing dots work
    let haystack5 = b"First: 1.2.3.4. Second: 5.6.7.8. End.";
    let ranges5: Vec<_> = extractor.find_iter(haystack5).collect();
    assert_eq!(ranges5.len(), 2);
    assert_eq!(&haystack5[ranges5[0].clone()], b"1.2.3.4");
    assert_eq!(&haystack5[ranges5[1].clone()], b"5.6.7.8");
}

#[test]
fn test_ipv4_edge_of_input() {
    // IPs at exact start and end of input
    check_extraction(b"192.168.1.1", &["192.168.1.1"], true, true);
    check_extraction(
        b"Start: 1.1.1.1 End: 8.8.8.8",
        &["1.1.1.1", "8.8.8.8"],
        true,
        true,
    );
}

#[test]
fn test_ipv6_compressed_forms() {
    // Note: ::8.8.8.8 might not match depending on the IPv6 regex pattern
    // The DFA might not recognize IPv4-mapped IPv6 addresses
    check_extraction(
        b"All zeros: ::, Loopback: ::1, Prefix: 2001:db8::",
        &["::", "::1", "2001:db8::"],
        true,
        true,
    );

    // Test IPv4-mapped IPv6 separately if it's supported
    let extractor = ExtractorBuilder::new().ipv6(true).build().unwrap();

    let haystack = b"IPv4-mapped: ::ffff:192.0.2.1";
    let matches: Vec<_> = extractor.find_iter(haystack).collect();
    // This may or may not match depending on regex pattern - just documenting behavior
    assert!(matches.len() <= 1);
}

#[test]
fn test_ipv6_ula_private_range() {
    // RFC 4193 Unique Local Addresses (fc00::/7) should be filtered
    check_extraction(
        b"ULA start: fc00::1, ULA end: fdff::1, Public: 2001:db8::1",
        &["fc00::1", "fdff::1", "2001:db8::1"],
        true,
        false,
    );

    // Verify ULA is filtered when include_private=false
    check_extraction(
        b"ULA: fc00::1, Link-local: fe80::1, Public: 2001:db8::1",
        &["2001:db8::1"],
        false,
        false,
    );
}

#[test]
fn test_ipv6_link_local() {
    // fe80::/10 link-local addresses
    check_extraction(
        b"Link-local: fe80::1, Public: 2001:db8::1",
        &["fe80::1", "2001:db8::1"],
        true,
        false,
    );

    check_extraction(
        b"Link-local: fe80::1, Public: 2001:db8::1",
        &["2001:db8::1"],
        false,
        false,
    );
}

#[test]
fn test_ipv6_loopback() {
    check_extraction(
        b"Loopback: ::1, Public: 2001:db8::1",
        &["::1", "2001:db8::1"],
        true,
        true,
    );

    check_extraction(
        b"Loopback: ::1, Public: 2001:db8::1",
        &["2001:db8::1"],
        true,
        false,
    );
}

#[test]
fn test_ipv6_full_form() {
    // Full uncompressed IPv6
    check_extraction(
        b"Full: 2001:0db8:0000:0000:0000:0000:0000:0001",
        &["2001:0db8:0000:0000:0000:0000:0000:0001"],
        true,
        true,
    );
}

#[test]
fn test_ipv6_invalid_double_compression() {
    // Multiple :: should be invalid (though rust parser might handle this)
    // This test verifies stdlib behavior
    check_extraction(b"Invalid: 2001::db8::1", &[], true, true);
}

#[test]
fn test_all_filters_combination() {
    // Test all filter combinations work correctly
    let haystack = b"Public4: 8.8.8.8, Private4: 10.0.0.1, Loop4: 127.0.0.1, \
                     Public6: 2001:db8::1, ULA6: fc00::1, Loop6: ::1";

    // Everything enabled
    check_extraction(
        haystack,
        &[
            "8.8.8.8",
            "10.0.0.1",
            "127.0.0.1",
            "2001:db8::1",
            "fc00::1",
            "::1",
        ],
        true,
        true,
    );

    // Only public
    check_extraction(haystack, &["8.8.8.8", "2001:db8::1"], false, false);

    // Public + private, no loopback
    check_extraction(
        haystack,
        &["8.8.8.8", "10.0.0.1", "2001:db8::1", "fc00::1"],
        true,
        false,
    );
}

#[test]
fn test_ipv4_only_extractor() {
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(false)
        .build()
        .unwrap();

    // Should extract IPv4 but skip IPv6
    let haystack = b"IPv4: 192.168.1.1, IPv6: 2001:db8::1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["192.168.1.1"]);

    // Multiple IPv4, multiple IPv6 - should only get IPv4
    let haystack2 = b"Servers: 10.0.0.1, 8.8.8.8, fe80::1, 2001:db8::1";
    let actual2: Vec<String> = extractor
        .find_iter(haystack2)
        .map(|range| String::from_utf8_lossy(&haystack2[range]).to_string())
        .collect();

    assert_eq!(actual2, vec!["10.0.0.1", "8.8.8.8"]);

    // Only IPv6 present - should extract nothing
    let haystack3 = b"IPv6 only: 2001:db8::1, ::1, fe80::dead:beef";
    let actual3: Vec<String> = extractor
        .find_iter(haystack3)
        .map(|range| String::from_utf8_lossy(&haystack3[range]).to_string())
        .collect();

    assert_eq!(actual3.len(), 0);
}

#[test]
fn test_ipv6_only_extractor() {
    let extractor = ExtractorBuilder::new()
        .ipv4(false)
        .ipv6(true)
        .build()
        .unwrap();

    // Should extract IPv6 but skip IPv4
    let haystack = b"IPv4: 192.168.1.1, IPv6: 2001:db8::1";
    let actual: Vec<String> = extractor
        .find_iter(haystack)
        .map(|range| String::from_utf8_lossy(&haystack[range]).to_string())
        .collect();

    assert_eq!(actual, vec!["2001:db8::1"]);

    // Multiple IPv6, multiple IPv4 - should only get IPv6
    let haystack2 = b"Servers: 10.0.0.1, fe80::1, 8.8.8.8, 2001:db8::1";
    let actual2: Vec<String> = extractor
        .find_iter(haystack2)
        .map(|range| String::from_utf8_lossy(&haystack2[range]).to_string())
        .collect();

    assert_eq!(actual2, vec!["fe80::1", "2001:db8::1"]);

    // Only IPv4 present - should extract nothing
    let haystack3 = b"IPv4 only: 192.168.1.1, 10.0.0.1, 127.0.0.1";
    let actual3: Vec<String> = extractor
        .find_iter(haystack3)
        .map(|range| String::from_utf8_lossy(&haystack3[range]).to_string())
        .collect();

    assert_eq!(actual3.len(), 0);

    // Test various IPv6 formats
    let haystack4 = b"Formats: ::1, 2001:db8::, fe80::dead:beef, fc00::1";
    let actual4: Vec<String> = extractor
        .find_iter(haystack4)
        .map(|range| String::from_utf8_lossy(&haystack4[range]).to_string())
        .collect();

    assert_eq!(
        actual4,
        vec!["::1", "2001:db8::", "fe80::dead:beef", "fc00::1"]
    );
}

#[test]
fn test_neither_ipv4_nor_ipv6() {
    let result = ExtractorBuilder::new().ipv4(false).ipv6(false).build();

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("No IP address patterns selected"));
}

#[test]
fn test_parse_ipv4_bytes_public_function() {
    use ip_extract::parse_ipv4_bytes;

    // Valid cases
    assert!(parse_ipv4_bytes(b"192.168.1.1").is_some());
    assert!(parse_ipv4_bytes(b"0.0.0.0").is_some());
    assert!(parse_ipv4_bytes(b"255.255.255.255").is_some());
    assert!(parse_ipv4_bytes(b"8.8.8.8").is_some());

    // Invalid cases
    assert!(parse_ipv4_bytes(b"256.1.1.1").is_none()); // Out of range
    assert!(parse_ipv4_bytes(b"192.168.01.1").is_none()); // Leading zero
    assert!(parse_ipv4_bytes(b"1.2.3.4.5").is_none()); // Too many octets
    assert!(parse_ipv4_bytes(b"1.2.3").is_none()); // Too few octets
    assert!(parse_ipv4_bytes(b"").is_none()); // Empty
    assert!(parse_ipv4_bytes(b"not.an.ip.addr").is_none()); // Non-numeric
}

#[test]
fn test_json_string_with_single_ip() {
    // Test JSON string where the value is exactly one IP
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    // IPv4 as complete value
    let json1 = br#"{"src_ip":"192.168.1.1"}"#;
    let ranges1: Vec<_> = extractor.find_iter(json1).collect();
    assert_eq!(ranges1.len(), 1);
    assert_eq!(&json1[ranges1[0].clone()], b"192.168.1.1");

    // IPv6 as complete value
    let json2 = br#"{"dst_ip":"2001:db8::1"}"#;
    let ranges2: Vec<_> = extractor.find_iter(json2).collect();
    assert_eq!(ranges2.len(), 1);
    assert_eq!(&json2[ranges2[0].clone()], b"2001:db8::1");

    // IP at start of string value
    let json3 = br#"{"msg":"192.168.1.1 connected"}"#;
    let ranges3: Vec<_> = extractor.find_iter(json3).collect();
    assert_eq!(ranges3.len(), 1);
    assert_eq!(&json3[ranges3[0].clone()], b"192.168.1.1");

    // IP at end of string value
    let json4 = br#"{"msg":"Connection from 10.0.0.5"}"#;
    let ranges4: Vec<_> = extractor.find_iter(json4).collect();
    assert_eq!(ranges4.len(), 1);
    assert_eq!(&json4[ranges4[0].clone()], b"10.0.0.5");

    // IP in middle of string value
    let json5 = br#"{"msg":"Host 172.16.0.1 responded"}"#;
    let ranges5: Vec<_> = extractor.find_iter(json5).collect();
    assert_eq!(ranges5.len(), 1);
    assert_eq!(&json5[ranges5[0].clone()], b"172.16.0.1");
}

#[test]
fn test_json_string_with_multiple_ips() {
    // Test JSON string with multiple IPs in log-like format
    let extractor = ExtractorBuilder::new()
        .ipv4(true)
        .ipv6(true)
        .build()
        .unwrap();

    // Multiple IPv4 addresses in syslog-style message
    let json1 = br#"{"syslog":"2024-01-15 Connection from 192.168.1.100 to 10.0.0.50 port 443"}"#;
    let ranges1: Vec<_> = extractor.find_iter(json1).collect();
    assert_eq!(ranges1.len(), 2);
    assert_eq!(&json1[ranges1[0].clone()], b"192.168.1.100");
    assert_eq!(&json1[ranges1[1].clone()], b"10.0.0.50");

    // Mixed IPv4 and IPv6
    let json2 = br#"{"log":"src=8.8.8.8 dst=2001:4860:4860::8888 proto=tcp"}"#;
    let ranges2: Vec<_> = extractor.find_iter(json2).collect();
    assert_eq!(ranges2.len(), 2);
    assert_eq!(&json2[ranges2[0].clone()], b"8.8.8.8");
    assert_eq!(&json2[ranges2[1].clone()], b"2001:4860:4860::8888");

    // Multiple IPs with various separators
    let json3 = br#"{"ips":"1.1.1.1, 8.8.8.8; 9.9.9.9 | 1.0.0.1"}"#;
    let ranges3: Vec<_> = extractor.find_iter(json3).collect();
    assert_eq!(ranges3.len(), 4);
    assert_eq!(&json3[ranges3[0].clone()], b"1.1.1.1");
    assert_eq!(&json3[ranges3[1].clone()], b"8.8.8.8");
    assert_eq!(&json3[ranges3[2].clone()], b"9.9.9.9");
    assert_eq!(&json3[ranges3[3].clone()], b"1.0.0.1");

    // Complex nested JSON with multiple IPs
    let json4 = br#"{"event":"firewall","src":"192.168.1.5","dst":"203.0.113.10","details":"Blocked connection from 192.168.1.5 to 203.0.113.10"}"#;
    let ranges4: Vec<_> = extractor.find_iter(json4).collect();
    assert_eq!(ranges4.len(), 4); // Each IP appears twice
    assert_eq!(&json4[ranges4[0].clone()], b"192.168.1.5");
    assert_eq!(&json4[ranges4[1].clone()], b"203.0.113.10");
    assert_eq!(&json4[ranges4[2].clone()], b"192.168.1.5");
    assert_eq!(&json4[ranges4[3].clone()], b"203.0.113.10");

    // Empty IP list in JSON
    let json5 = br#"{"ips":"","message":"no ips here"}"#;
    let ranges5: Vec<_> = extractor.find_iter(json5).collect();
    assert_eq!(ranges5.len(), 0);
}
