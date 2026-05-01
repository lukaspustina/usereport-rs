//! Integration tests for SDD `specs/sdd/version-2.md` Phase 0 (v1.5 cleanup).
//!
//! Each `#[test]` corresponds to one numbered acceptance criterion in
//! `specs/sdd/version-2.md` Phase 0 (lines 541–568) and the related Test
//! Scenarios block.
#![cfg(feature = "bin")]

use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use googletest::prelude::*;
use toml::Value as Toml;

use usereport::cli::OutputType;
use usereport::{Command, CommandResult};

const REPO_ROOT: &str = env!("CARGO_MANIFEST_DIR");

fn read_repo_file(rel: &str) -> String {
    let path = Path::new(REPO_ROOT).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {}", rel, e))
}

fn parse_toml(rel: &str) -> Toml {
    let content = read_repo_file(rel);
    toml::from_str(&content).unwrap_or_else(|e| panic!("failed to parse {} as TOML: {}", rel, e))
}

// ---------------------------------------------------------------------------
// Criterion 1 — CommandResult::SkippedMissing
// ---------------------------------------------------------------------------

#[test]
fn criterion_1_exec_returns_skipped_missing_when_binary_absent() {
    let bogus = "definitely_not_a_real_binary_xyz789";
    let cmd = Command::new(bogus, bogus);
    let res = cmd.exec();
    let expected = bogus.to_string();
    assert_that!(
        res,
        matches_pattern!(CommandResult::SkippedMissing {
            binary: eq(&expected),
            ..
        })
    );
}

#[test]
fn criterion_1_exec_skipped_missing_distinct_from_error() {
    let bogus = "definitely_not_a_real_binary_abc123";
    let cmd = Command::new(bogus, bogus);
    let res = cmd.exec();
    match res {
        CommandResult::SkippedMissing { .. } => {}
        CommandResult::Error { .. } => {
            panic!("expected SkippedMissing for missing PATH binary, got Error");
        }
        other => panic!("unexpected variant: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Criterion 2 — OutputType::Hbs renamed to OutputType::Template; "hbs" alias
// ---------------------------------------------------------------------------

#[test]
fn criterion_2_from_str_template_returns_template() {
    let res = OutputType::from_str("template").expect("template parses");
    assert_eq!(res, OutputType::Template);
}

#[test]
fn criterion_2_from_str_hbs_alias_returns_template() {
    let res = OutputType::from_str("hbs").expect("hbs parses as alias");
    assert_eq!(res, OutputType::Template);
}

#[test]
fn criterion_2_from_str_unknown_returns_err() {
    assert_that!(OutputType::from_str("flubber"), err(anything()));
}

// ---------------------------------------------------------------------------
// Criterion 3 — `-O, --output-file <PATH>` writes to file with parent dirs
// ---------------------------------------------------------------------------

#[test]
fn criterion_3_output_writer_creates_parent_dirs() {
    use usereport::cli::output_writer;

    let tmp = tempfile::tempdir().expect("create tempdir");
    let path = tmp.path().join("nested/sub/dir/report.out");
    {
        let mut w = output_writer(&Some(path.clone())).expect("output_writer ok");
        w.write_all(b"hello").unwrap();
        w.flush().unwrap();
    }
    assert!(path.exists(), "output file should exist at {:?}", path);
    let content = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content, "hello");
}

#[test]
fn criterion_3_output_writer_none_returns_writer() {
    use usereport::cli::output_writer;

    // None means stdout — verify the function returns Ok without writing
    // (writing to stdout would pollute test output).
    let _ = output_writer(&None).expect("output_writer ok for stdout");
}

// ---------------------------------------------------------------------------
// Criterion 4 — HTML template fully offline (no third-party CDNs)
// ---------------------------------------------------------------------------

#[test]
fn criterion_4_html_template_has_no_remote_css_or_js() {
    let html = read_repo_file("contrib/html.j2");
    let lower = html.to_lowercase();
    assert!(
        !lower.contains("bootstrapcdn.com"),
        "html.j2 must not link to bootstrapcdn.com"
    );
    assert!(
        !lower.contains("cdn.jsdelivr.net"),
        "html.j2 must not link to cdn.jsdelivr.net"
    );
    assert!(
        !lower.contains("code.jquery.com"),
        "html.j2 must not link to code.jquery.com"
    );

    for line in html.lines() {
        let l = line.to_lowercase();
        let is_remote_link = l.contains("<link") && (l.contains("href=\"http") || l.contains("href='http"));
        let is_remote_script = l.contains("<script") && (l.contains("src=\"http") || l.contains("src='http"));
        assert!(!is_remote_link, "html.j2 has a remote stylesheet link: {}", line);
        assert!(!is_remote_script, "html.j2 has a remote script src: {}", line);
    }
}

// ---------------------------------------------------------------------------
// Criterion 5 — Linux config uses `ss` and bare `$PATH` binaries
// ---------------------------------------------------------------------------

#[test]
fn criterion_5_linux_config_no_absolute_paths_in_commands() {
    let conf = parse_toml("contrib/linux.conf");
    let commands = conf
        .get("command")
        .and_then(Toml::as_array)
        .expect("[[command]] array present");
    let bad: Vec<String> = commands
        .iter()
        .filter_map(|c| {
            let cmd_str = c.get("command").and_then(Toml::as_str)?;
            let first = cmd_str.split_whitespace().next()?;
            if first.starts_with("/usr/bin/")
                || first.starts_with("/bin/")
                || first.starts_with("/sbin/")
                || first.starts_with("/usr/sbin/")
            {
                Some(format!(
                    "{}: {}",
                    c.get("name").and_then(Toml::as_str).unwrap_or("?"),
                    cmd_str
                ))
            } else {
                None
            }
        })
        .collect();
    assert!(
        bad.is_empty(),
        "linux.conf commands must not use absolute /usr/bin or /bin paths; found: {:?}",
        bad
    );
}

#[test]
fn criterion_5_linux_config_no_netstat() {
    let conf = parse_toml("contrib/linux.conf");
    let commands = conf
        .get("command")
        .and_then(Toml::as_array)
        .expect("[[command]] array present");
    for c in commands {
        let name = c.get("name").and_then(Toml::as_str).unwrap_or("");
        let cmd = c.get("command").and_then(Toml::as_str).unwrap_or("");
        assert_ne!(
            name, "netstat_all_tcp",
            "linux.conf still has the deprecated `netstat_all_tcp` command"
        );
        let has_netstat = cmd
            .split_whitespace()
            .any(|t| t.trim_matches('\'').trim_matches('"') == "netstat");
        assert!(
            !has_netstat,
            "linux.conf still references `netstat`: {} -> {}",
            name, cmd
        );
    }
}

// ---------------------------------------------------------------------------
// Criterion 6 — macOS config has `mem` and `net` profiles, no echo placeholders
// ---------------------------------------------------------------------------

#[test]
fn criterion_6_osx_config_has_mem_and_net_profiles() {
    let conf = parse_toml("contrib/osx.conf");
    let profiles = conf
        .get("profile")
        .and_then(Toml::as_array)
        .expect("[[profile]] array present");
    let names: Vec<&str> = profiles
        .iter()
        .filter_map(|p| p.get("name").and_then(Toml::as_str))
        .collect();
    assert!(
        names.contains(&"default"),
        "osx.conf must declare `default` profile (found {:?})",
        names
    );
    assert!(
        names.contains(&"mem"),
        "osx.conf must declare `mem` profile (found {:?})",
        names
    );
    assert!(
        names.contains(&"net"),
        "osx.conf must declare `net` profile (found {:?})",
        names
    );
}

#[test]
fn criterion_6_osx_config_no_echo_placeholders() {
    let conf = parse_toml("contrib/osx.conf");
    let commands = conf
        .get("command")
        .and_then(Toml::as_array)
        .expect("[[command]] array present");
    for c in commands {
        let name = c.get("name").and_then(Toml::as_str).unwrap_or("?");
        let cmd = c.get("command").and_then(Toml::as_str).unwrap_or("");
        let first = cmd.split_whitespace().next().unwrap_or("");
        assert!(
            !matches!(first, "echo" | "/bin/echo" | "/usr/bin/echo"),
            "osx.conf command `{}` is an `echo` placeholder: {}",
            name,
            cmd
        );
    }
}

// ---------------------------------------------------------------------------
// Criterion 7 — Cargo.toml include covers contrib/rules and contrib/patterns
// ---------------------------------------------------------------------------

#[test]
fn criterion_7_cargo_include_covers_rules_and_patterns() {
    let cargo = parse_toml("Cargo.toml");
    let include = cargo
        .get("package")
        .and_then(|p| p.get("include"))
        .and_then(Toml::as_array)
        .expect("package.include array present");
    let entries: Vec<&str> = include.iter().filter_map(Toml::as_str).collect();
    assert!(
        entries.contains(&"contrib/rules/**/*.toml"),
        "Cargo.toml include missing `contrib/rules/**/*.toml`; got {:?}",
        entries
    );
    assert!(
        entries.contains(&"contrib/patterns/**/*.toml"),
        "Cargo.toml include missing `contrib/patterns/**/*.toml`; got {:?}",
        entries
    );
}

// ---------------------------------------------------------------------------
// Criterion 8 — Repo hygiene: rules/patterns dirs, CI workflow, README
// ---------------------------------------------------------------------------

#[test]
fn criterion_8a_contrib_rules_and_patterns_dirs_exist() {
    let rules = Path::new(REPO_ROOT).join("contrib/rules");
    let patterns = Path::new(REPO_ROOT).join("contrib/patterns");
    assert!(rules.is_dir(), "contrib/rules/ directory must exist");
    assert!(patterns.is_dir(), "contrib/patterns/ directory must exist");
}

#[test]
fn criterion_8b_github_workflows_ci_yml_has_required_steps() {
    let ci = read_repo_file(".github/workflows/ci.yml");
    assert!(ci.contains("cargo fmt"), "ci.yml must run `cargo fmt`");
    assert!(ci.contains("cargo clippy"), "ci.yml must run `cargo clippy`");
    assert!(ci.contains("cargo test"), "ci.yml must run `cargo test`");
    assert!(ci.contains("cargo audit"), "ci.yml must run `cargo audit`");
}

#[test]
fn criterion_8c_readme_dropped_azure_and_bionic_added_binstall_and_jinja() {
    let readme = read_repo_file("README.md");
    assert!(
        !readme.contains("dev.azure.com"),
        "README must no longer reference Azure DevOps"
    );
    assert!(
        !readme.contains("Bionic"),
        "README must no longer reference Ubuntu Bionic"
    );
    assert!(
        readme.to_lowercase().contains("cargo binstall"),
        "README must mention `cargo binstall`"
    );
    assert!(readme.contains("Jinja2"), "README must mention `Jinja2`");
}

#[test]
fn criterion_8d_azure_pipelines_archived_or_removed() {
    let original = Path::new(REPO_ROOT).join(".ci/azure-pipelines.yml");
    assert!(
        !original.exists(),
        "{} must be removed or archived (renamed to .archived)",
        original.display()
    );
}
