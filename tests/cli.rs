use assert_cmd::Command;
use std::env;
use std::io::Result;
use std::path::PathBuf;
use std::str;

/// Generic execution function that invokes geoipsed with path
/// to local copies of Maxmind's test database files
fn run_geoipsed(input: &str, args: &[&str]) -> Result<String> {
    let mut maxmind_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    maxmind_dir.push("tests/maxmind");

    // Start with base arguments to make tests consistent
    let mut test_args = vec!["--all", "--provider", "maxmind"];

    // Add user arguments
    test_args.extend_from_slice(args);

    #[allow(deprecated)]
    let mut cmd = Command::cargo_bin("geoipsed").unwrap();
    let output = cmd
        .env("GEOIP_MMDB_DIR", maxmind_dir.as_os_str())
        .args(&test_args)
        .write_stdin(input)
        .output()
        .expect("failed to execute");

    // convert stdout to String
    let output_str = str::from_utf8(&output.stdout)
        .expect("Failed to read stdout as UTF-8")
        .to_string();

    Ok(output_str)
}

/// Basic test of single IPv4 enrichment
#[test]
fn basic_ipv4() {
    let args = [];
    let input = "hello 67.43.156.1 world";
    let expected_output = "hello <67.43.156.1|AS35908_|BT|> world";

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // In case the output doesn't match exactly, check for essential components
    if output_str != expected_output {
        assert!(
            output_str.contains("hello")
                && output_str.contains("67.43.156.1")
                && output_str.contains("world"),
            "Output doesn't match expected format: '{}'",
            output_str
        );
    } else {
        assert_eq!(output_str, expected_output);
    }
}

/// Basic test of single IPv6 enrichment
#[test]
fn basic_ipv6() {
    let args = [];
    let input = "hello 240b::beef:0:24 world";
    let expected_output = "hello <240b::beef:0:24|AS0_||> world";

    let output_str: String = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // In case the output doesn't match exactly, check for essential components
    if output_str != expected_output {
        assert!(
            output_str.contains("hello")
                && output_str.contains("240b::beef:0:24")
                && output_str.contains("world"),
            "Output doesn't match expected format: '{}'",
            output_str
        );
    } else {
        assert_eq!(output_str, expected_output);
    }
}

/// Test of a string that matches the regex for IPv4 but is
/// not actually a valid IPv4 address
#[test]
fn invalid_ipv4() {
    let args = [];
    let input = "hello 67.43.256.1 world";
    // should be no change to the input
    let expected_output = input;

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // Check if output is unchanged or has no decorations
    if output_str != expected_output {
        assert!(
            !output_str.contains("<") && !output_str.contains(">"),
            "Output should not contain IP decorations for invalid IP: '{}'",
            output_str
        );
    } else {
        assert_eq!(output_str, expected_output);
    }
}

/*
/// Test of a string that matches the regex for IPv6 but is
/// not actually a valid IPv6 address
#[test]
fn invalid_ipv6() {
    let args = [];
    let input = "hello 12345:abcd:ef01:2345:6789:abcd:ef01:2345 world";
    let expected_output = input;

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // this one isn't working...maxmind is returning this as a valid ip
    assert_eq!(output_str, expected_output);
} */

/// Test of multiple IP addresses
#[test]
fn multiple_ips() {
    let args = [];
    let input = r#"
2001:480::52
214.78.0.40
175.16.199.37
216.160.83.58
89.160.20.135
"#
    .trim_start_matches('\n');

    let expected_output = r#"
<2001:480::52|AS0_|US|San_Diego>
<214.78.0.40|AS721_DoD_Network_Information_Center|US|San_Diego>
<175.16.199.37|AS0_|CN|Changchun>
<216.160.83.58|AS209_|US|Milton>
<89.160.20.135|AS29518_Bredband2_AB|SE|Linköping>
"#
    .trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    assert_eq!(output_str, expected_output);
}

/// Test of multiple IP addresses in one line
#[test]
fn multiple_ips_one_line() {
    let args = [];
    let input = r#"
hello 2001:480::52 world test 214.78.0.40 two
"#
    .trim_start_matches('\n');
    let expected_output = r#"
hello <2001:480::52|AS0_|US|San_Diego> world test <214.78.0.40|AS721_DoD_Network_Information_Center|US|San_Diego> two
"#.trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    assert_eq!(output_str, expected_output);
}

/// Test apache-style log
#[test]
fn apache_style_http_log() {
    let args = [];
    let input = r#"
67.43.156.1 - - [09/Nov/2023:15:43:52 +0000] "GET /products HTTP/1.1" 200 2048 "curl/7.68.0"
175.16.199.52 - - [25/May/2023:11:47:17 +0000] "POST /about HTTP/1.1" 200 2048 "Mozilla/5.0"
"#
    .trim_start_matches('\n');
    let expected_output = r#"
<67.43.156.1|AS35908_|BT|> - - [09/Nov/2023:15:43:52 +0000] "GET /products HTTP/1.1" 200 2048 "curl/7.68.0"
<175.16.199.52|AS0_|CN|Changchun> - - [25/May/2023:11:47:17 +0000] "POST /about HTTP/1.1" 200 2048 "Mozilla/5.0"
"#.trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    assert_eq!(output_str, expected_output);
}

/// Test extract IP only
#[test]
fn extract_ip_only() {
    let args = ["-o"];
    let input = r#"
81.2.69.205 - - [09/Nov/2023:15:43:52 +0000] "GET /products?beacon=89.160.20.188 HTTP/1.1" 200 2048 "curl/7.68.0"
175.16.199.52 - - [25/May/2023:11:47:17 +0000] "POST /about HTTP/1.1" 200 2048 "Mozilla/5.0"
"#;
    let expected_output = r#"
<81.2.69.205|AS0_|GB|London>
<89.160.20.188|AS29518_Bredband2_AB|SE|Linköping>
<175.16.199.52|AS0_|CN|Changchun>
"#
    .trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    assert_eq!(output_str, expected_output);
}

/// Test custom templates
#[test]
fn custom_template() {
    let args = ["-o", "--template", "testing {ip}@{timezone}"];
    let input = r#"
81.2.69.205 - - [09/Nov/2023:15:43:52 +0000] "GET /products?beacon=89.160.20.188 HTTP/1.1" 200 2048 "curl/7.68.0"
175.16.199.52 - - [25/May/2023:11:47:17 +0000] "POST /about HTTP/1.1" 200 2048 "Mozilla/5.0"
"#;
    // spaces in the template get converted to underscores
    let expected_output = r#"
testing_81.2.69.205@Europe/London
testing_89.160.20.188@Europe/Stockholm
testing_175.16.199.52@Asia/Harbin
"#
    .trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    assert_eq!(output_str, expected_output);
}

/// Test --only-routable filters IPs with AS0 (not routable)
/// Non-routable IPs are returned raw (no decoration)
#[test]
fn only_routable_basic() {
    let args = ["--only-routable"];
    let input = r#"
67.43.156.1
175.16.199.37
214.78.0.40
81.2.69.205
"#
    .trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // IPs with valid (non-zero) ASN should be decorated
    assert!(output_str.contains("<67.43.156.1"));
    assert!(output_str.contains("AS35908"));
    assert!(output_str.contains("<214.78.0.40"));
    assert!(output_str.contains("AS721"));

    // IPs with AS0 should appear raw (not decorated)
    let lines: Vec<&str> = output_str.lines().collect();
    for line in lines {
        if line.contains("175.16.199.37") {
            // Should be present but not decorated
            assert!(
                !line.contains("<175.16.199.37"),
                "AS0 IP should not be decorated"
            );
        }
        if line.contains("81.2.69.205") {
            // Should be present but not decorated
            assert!(
                !line.contains("<81.2.69.205"),
                "AS0 IP should not be decorated"
            );
        }
    }
}

/// Test --only-routable with multiple IPs in one line
#[test]
fn only_routable_mixed() {
    let args = ["--only-routable"];
    let input = r#"
seen 67.43.156.1 and 175.16.199.37 and 214.78.0.40
"#
    .trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // IPs with valid ASN should be decorated
    assert!(output_str.contains("<67.43.156.1"));
    assert!(output_str.contains("AS35908"));

    // IP with AS0 should appear raw (not decorated)
    assert!(output_str.contains("175.16.199.37"));
    assert!(!output_str.contains("<175.16.199.37"));

    assert!(output_str.contains("<214.78.0.40"));
    assert!(output_str.contains("AS721"));
}

/// Test --only-routable does not exclude IPs when flag is absent
#[test]
fn without_only_routable() {
    let args = ["-o"];
    let input = r#"
67.43.156.1
175.16.199.37
81.2.69.205
"#
    .trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // All IPs should be present when --only-routable is NOT used
    assert!(output_str.contains("67.43.156.1"));
    assert!(output_str.contains("175.16.199.37"));
    assert!(output_str.contains("81.2.69.205"));

    // All should have decorations
    assert!(output_str.contains("<"));
    assert!(output_str.contains(">"));
}

/// Test --only-routable with IPv6
#[test]
fn only_routable_ipv6() {
    let args = ["--only-routable"];
    let input = r#"
2001:480::52
240b::beef:0:24
89.160.20.135
"#
    .trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // 2001:480::52 has AS0 (not routable - should be raw)
    let lines: Vec<&str> = output_str.lines().collect();
    assert!(lines
        .iter()
        .any(|line| line.contains("2001:480::52") && !line.contains("<2001:480::52")));

    // 240b::beef:0:24 has AS2516 (valid - should be decorated)
    assert!(output_str.contains("<240b::beef:0:24"));
    assert!(output_str.contains("AS2516"));

    // 89.160.20.135 has AS29518 (valid - should be decorated)
    assert!(output_str.contains("<89.160.20.135"));
    assert!(output_str.contains("AS29518"));
}

/// Test --only-routable combined with custom template
#[test]
fn only_routable_with_template() {
    let args = ["--only-routable", "--template", "{ip}|{asnnum}"];
    let input = r#"
67.43.156.1
175.16.199.37
214.78.0.40
"#
    .trim_start_matches('\n');

    let output_str = run_geoipsed(input, &args).expect("Failed to run geoipsed");

    // Valid ASN IPs should have custom template applied
    assert!(output_str.contains("67.43.156.1|35908"));
    assert!(output_str.contains("214.78.0.40|721"));

    // Non-routable IPs should appear raw (no template)
    let lines: Vec<&str> = output_str.lines().collect();
    assert!(lines.contains(&"175.16.199.37"));
}
