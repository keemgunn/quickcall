use crate::common::unique_scratch;
use crate::package_support::resolve_target_json;

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_no_home_qc(home: &std::path::Path) {
    assert!(
        !home.join(".qc").exists(),
        "selector must not create {}",
        home.join(".qc").display()
    );
}

#[test]
fn selector_maps_all_six_supported_triples() {
    let scratch = unique_scratch("sel-six");
    let cases = [
        ("darwin", "arm64", None, "aarch64-apple-darwin"),
        ("darwin", "x64", None, "x86_64-apple-darwin"),
        ("linux", "x64", Some("glibc"), "x86_64-unknown-linux-gnu"),
        ("linux", "arm64", Some("glibc"), "aarch64-unknown-linux-gnu"),
        ("linux", "x64", Some("musl"), "x86_64-unknown-linux-musl"),
        ("linux", "arm64", Some("musl"), "aarch64-unknown-linux-musl"),
    ];
    for (platform, arch, libc, expected) in cases {
        let output = resolve_target_json(platform, arch, libc, &scratch.home());
        assert_eq!(
            output.status.code(),
            Some(0),
            "{platform}/{arch}/{libc:?} stderr={}",
            stderr(&output)
        );
        assert_eq!(stdout(&output), expected);
        assert_no_home_qc(&scratch.home());
    }
}

#[test]
fn selector_rejects_unsupported_and_ambiguous_without_touching_home() {
    let scratch = unique_scratch("sel-fail");
    let cases = [
        ("win32", "x64", None, "unsupported"),
        ("linux", "x64", None, "ambiguous"),
        ("linux", "x64", Some(""), "ambiguous"),
        ("linux", "x64", Some("gnu"), "ambiguous"),
        ("linux", "ia32", Some("glibc"), "unsupported"),
        ("darwin", "ia32", None, "unsupported"),
        ("freebsd", "x64", None, "unsupported"),
    ];
    for (platform, arch, libc, kind) in cases {
        let output = resolve_target_json(platform, arch, libc, &scratch.home());
        assert_ne!(
            output.status.code(),
            Some(0),
            "{platform}/{arch}/{libc:?} should fail ({kind})"
        );
        assert!(stdout(&output).is_empty());
        let err = stderr(&output);
        assert!(
            err.starts_with("qc:"),
            "packaging error on stderr, got {err:?}"
        );
        assert_no_home_qc(&scratch.home());
    }
}
