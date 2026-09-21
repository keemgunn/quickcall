mod bootstrap;
mod merge;
mod parse;
mod paths;

pub use bootstrap::{
    BootstrapMode, REFERENCE_GITIGNORE_ENTRIES, bootstrap_global, bootstrap_settings,
    hard_refresh_sample_prompts, install_sample_prompts,
};
pub use merge::{effective_shell, load_config};
pub use parse::{Config, ToolConfig, ToolDefaults, read_config};
pub use paths::{ConfigPaths, config_paths, fallback_home, home_from_env, resolve_bootstrap_home};
