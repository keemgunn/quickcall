use std::path::PathBuf;
use std::process::Command;

use crate::common::{host_triple, unique_scratch};
use crate::package_support::{
    NATIVE_TARGETS, assert_checksum, is_host_mode, npm_install_tarball, npm_pack, package_mode,
    pnpm_install_tarball, run_launcher, sha256_hex, stage_installable_layout, tarball_paths,
};

#[test]
fn host_tarball_contains_only_host_native_pair() {
    if !is_host_mode() {
        return;
    }
    let scratch = unique_scratch("tgz-list");
    let layout = scratch.root.join("layout");
    stage_installable_layout(&layout, "0.0.0");
    let packed = npm_pack(&layout, &scratch.root.join("pack"), &scratch.home());
    let listing = tarball_paths(&packed);
    let triple = host_triple();
    let qc = format!("package/dist/{triple}/qc");
    let bootstrap = format!("package/dist/{triple}/qc-bootstrap");
    assert!(
        listing.iter().any(|p| p == &qc),
        "missing {qc} in {listing:?}"
    );
    assert!(
        listing.iter().any(|p| p == &bootstrap),
        "missing {bootstrap}"
    );
    assert!(listing.iter().any(|p| p == "package/bin/qc"));
    assert!(listing.iter().any(|p| p == "package/scripts/native.mjs"));
    assert!(
        listing
            .iter()
            .any(|p| p == "package/scripts/postinstall.mjs")
    );
    assert!(
        listing
            .iter()
            .any(|p| p.contains("package/share/settings/config.toml"))
    );
    for path in &listing {
        assert!(!path.contains("qc-test-agent"), "{path}");
        assert!(!path.contains("/tests/"), "{path}");
        assert!(!path.contains("Cargo.toml"), "{path}");
        assert!(!path.contains("target/"), "{path}");
        assert!(!path.ends_with("cli.js"), "{path}");
    }
    let dist_bins: Vec<_> = listing
        .iter()
        .filter(|path| {
            let rest = path.strip_prefix("package/dist/").unwrap_or("");
            rest.ends_with("/qc") || rest.ends_with("/qc-bootstrap")
        })
        .collect();
    assert_eq!(
        dist_bins.len(),
        2,
        "host-only native pair, got {dist_bins:?}"
    );
}

#[test]
fn host_npm_install_skip_scripts_still_runs_version() {
    if !is_host_mode() {
        return;
    }
    let scratch = unique_scratch("tgz-npm-skip");
    let layout = scratch.root.join("layout");
    stage_installable_layout(&layout, "3.1.4");
    let packed = npm_pack(&layout, &scratch.root.join("pack"), &scratch.home());
    let installed = npm_install_tarball(
        &packed,
        &scratch.root.join("npm-prefix"),
        &scratch.home(),
        true,
    );
    assert!(!scratch.home().join(".qc").exists());
    let output = run_launcher(
        &installed.bin_qc,
        &scratch,
        &scratch.cwd(),
        &["--version"],
        &[],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "3.1.4\n");
    assert!(
        installed
            .package_root
            .join("share/settings/config.toml")
            .is_file()
    );
    assert!(!installed.package_root.join("Cargo.toml").exists());
    assert!(
        !installed
            .package_root
            .join("dist")
            .join(host_triple())
            .join("qc-test-agent")
            .exists()
    );
}

#[test]
fn host_npm_install_runs_postinstall_into_isolated_home() {
    if !is_host_mode() {
        return;
    }
    let scratch = unique_scratch("tgz-npm-life");
    let layout = scratch.root.join("layout");
    stage_installable_layout(&layout, "0.0.0");
    let packed = npm_pack(&layout, &scratch.root.join("pack"), &scratch.home());
    let _installed = npm_install_tarball(
        &packed,
        &scratch.root.join("npm-prefix"),
        &scratch.home(),
        false,
    );
    // npm v12 may skip lifecycle scripts unless allowlisted; run the shipped file if needed.
    if !scratch.home().join(".qc/config.toml").is_file() {
        let script = _installed.package_root.join("scripts/postinstall.mjs");
        let repair = Command::new(crate::common::require_cmd("node"))
            .arg(&script)
            .env("HOME", scratch.home())
            .output()
            .expect("postinstall");
        assert_eq!(
            repair.status.code(),
            Some(0),
            "stderr={}",
            String::from_utf8_lossy(&repair.stderr)
        );
    }
    assert!(scratch.home().join(".qc/config.toml").is_file());
}

#[test]
fn host_pnpm_global_prefix_is_isolated() {
    if !is_host_mode() {
        return;
    }
    let scratch = unique_scratch("tgz-pnpm");
    let layout = scratch.root.join("layout");
    stage_installable_layout(&layout, "2.2.2");
    let packed = npm_pack(&layout, &scratch.root.join("pack"), &scratch.home());
    let Some(installed) = pnpm_install_tarball(
        &packed,
        &scratch.root.join("pnpm-prefix"),
        &scratch.home(),
        true,
    ) else {
        panic!("pnpm must be on PATH for host-mode package tests");
    };
    assert!(installed.bin_qc.exists());
    let output = run_launcher(
        &installed.bin_qc,
        &scratch,
        &scratch.cwd(),
        &["--version"],
        &[],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "2.2.2\n");
}

#[test]
fn sha256_helper_matches_known_file() {
    let scratch = unique_scratch("sha256-known");
    let path = scratch.root.join("blob");
    std::fs::write(&path, b"quickcall-checksum-fixture\n").expect("write");
    assert_eq!(
        sha256_hex(&path),
        "fb5b6e2e86005fb694fb32f48e7a15efc553213e875e41ad4e33a99eb731bae7"
    );
}

#[test]
fn final_artifact_requires_twelve_binaries_and_checksum() {
    if package_mode() != "final-artifact" {
        return;
    }
    let artifact = PathBuf::from(std::env::var("QC_PACKAGE_ARTIFACT").expect("artifact"));
    let expected = std::env::var("QC_PACKAGE_CHECKSUM").expect("checksum");
    assert!(
        artifact.is_absolute(),
        "artifact path must be absolute: {}",
        artifact.display()
    );
    assert!(
        artifact.is_file(),
        "missing artifact {}",
        artifact.display()
    );
    assert_checksum(&artifact, &expected);

    let listing = tarball_paths(&artifact);
    for triple in NATIVE_TARGETS {
        for bin in ["qc", "qc-bootstrap"] {
            let path = format!("package/dist/{triple}/{bin}");
            assert!(
                listing.iter().any(|p| p == &path),
                "missing {path} in {listing:?}"
            );
        }
    }
    for path in &listing {
        assert!(!path.contains("qc-test-agent"), "{path}");
        assert!(!path.ends_with("cli.js"), "{path}");
    }
    let dist_bins: Vec<_> = listing
        .iter()
        .filter(|path| {
            let rest = path.strip_prefix("package/dist/").unwrap_or("");
            rest.ends_with("/qc") || rest.ends_with("/qc-bootstrap")
        })
        .collect();
    assert_eq!(dist_bins.len(), 12, "twelve executables, got {dist_bins:?}");
}

#[test]
fn final_artifact_installs_host_pair_without_rebuild() {
    if package_mode() != "final-artifact" {
        return;
    }
    let artifact = PathBuf::from(std::env::var("QC_PACKAGE_ARTIFACT").expect("artifact"));
    let expected = std::env::var("QC_PACKAGE_CHECKSUM").expect("checksum");
    assert_checksum(&artifact, &expected);

    let scratch = unique_scratch("final-art-install");
    let installed = npm_install_tarball(
        &artifact,
        &scratch.root.join("npm-prefix"),
        &scratch.home(),
        true,
    );
    let host = host_triple();
    assert!(
        installed
            .package_root
            .join("dist")
            .join(host)
            .join("qc")
            .is_file()
    );
    assert!(
        installed
            .package_root
            .join("dist")
            .join(host)
            .join("qc-bootstrap")
            .is_file()
    );
    let output = run_launcher(
        &installed.bin_qc,
        &scratch,
        &scratch.cwd(),
        &["--version"],
        &[],
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.is_empty());
}

#[test]
fn final_artifact_rejects_wrong_checksum() {
    if package_mode() != "final-artifact" {
        return;
    }
    let artifact = PathBuf::from(std::env::var("QC_PACKAGE_ARTIFACT").expect("artifact"));
    let got = sha256_hex(&artifact);
    let bogus = "0".repeat(64);
    assert_ne!(got, bogus);
    let result = std::panic::catch_unwind(|| {
        assert_checksum(&artifact, &bogus);
    });
    assert!(result.is_err(), "wrong checksum must fail");
}
