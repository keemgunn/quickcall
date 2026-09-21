//! Native bootstrap helper. Public `qc` grammar is unchanged; this binary is
//! invoked by package/install helpers, not registered as a command.
//!
//! `qc-bootstrap settings repair|refresh`
//! `qc-bootstrap harness refresh-known`

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

use quickcall::config::{
    BootstrapMode, ConfigPaths, bootstrap_settings, config_paths, fallback_home,
    resolve_bootstrap_home,
};
use quickcall::harness::refresh_known;
use quickcall::messages::{BOOTSTRAP_USAGE, error};
use quickcall::package::{PackageLayout, resolve_package};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    match run(&argv) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run(argv: &[String]) -> Result<(), ExitCode> {
    match argv {
        [kind, mode] if kind == "settings" && (mode == "repair" || mode == "refresh") => {
            let bootstrap_mode = if mode.as_str() == "refresh" {
                BootstrapMode::Refresh
            } else {
                BootstrapMode::Repair
            };
            let (paths, _, _) = helper_context()?;
            bootstrap_settings(bootstrap_mode, &paths).map_err(fail)
        }
        [kind, mode] if kind == "harness" && mode == "refresh-known" => {
            let (_, home, package) = helper_context()?;
            refresh_known(&home, &package.bundled_skills).map_err(fail)?;
            Ok(())
        }
        _ => {
            eprintln!("{BOOTSTRAP_USAGE}");
            Err(ExitCode::from(1))
        }
    }
}

fn helper_context() -> Result<(ConfigPaths, PathBuf, PackageLayout), ExitCode> {
    let cwd = std::env::current_dir().map_err(|cause| {
        eprintln!("{}", error(&format!("cannot read cwd: {cause}")));
        ExitCode::from(1)
    })?;
    let executable = std::env::current_exe().map_err(|cause| {
        eprintln!("{}", error(&format!("cannot resolve executable: {cause}")));
        ExitCode::from(1)
    })?;
    let env: HashMap<String, String> = std::env::vars().collect();
    let home = resolve_bootstrap_home(&env, fallback_home).map_err(fail)?;
    let package = resolve_package(&executable).map_err(fail)?;
    Ok((
        config_paths(&home, &cwd, &package.starter_config),
        home,
        package,
    ))
}

fn fail(cause: quickcall::QcError) -> ExitCode {
    eprintln!("{}", error(&cause.message));
    ExitCode::from(u8::try_from(cause.exit_code).unwrap_or(1))
}
