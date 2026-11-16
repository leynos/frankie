//! Shared test helpers for configuration tests.

use std::ffi::OsString;
use std::sync::Arc;

use ortho_config::{MapEnv, MergeComposer, OrthoError};
use serde_json::Value;

use crate::FrankieConfig;

/// Loads a [`FrankieConfig`] from `cli_args` and the closed environment `vars`.
///
/// The environment is a [`MapEnv`] that pins the working directory to a fresh
/// temporary directory and names `HOME`, `XDG_CONFIG_HOME` and
/// `XDG_CONFIG_DIRS` as that same empty directory, so no host configuration
/// file contributes (an unset `HOME` falls back to the host and an empty
/// `XDG_CONFIG_DIRS` selects `/etc/xdg`). Nothing reads or writes the process
/// environment or working directory, so callers run concurrently. A `vars`
/// entry whose value is `None` is left out of the map.
///
/// # Errors
///
/// Returns an error when the temporary directory cannot be created or the
/// configuration does not load; the calling test owns every assertion.
pub fn load_config_in_isolated_env(
    vars: &[(&str, Option<&str>)],
    cli_args: &[&str],
) -> Result<FrankieConfig, Box<dyn std::error::Error>> {
    let temp_dir = tempfile::TempDir::new()?;
    let home = temp_dir.path().to_string_lossy().to_string();

    let mut env = MapEnv::new()
        .with_current_dir(temp_dir.path())
        .with_var("HOME", home.as_str())
        .with_var("XDG_CONFIG_HOME", home.as_str())
        .with_var("XDG_CONFIG_DIRS", home.as_str());
    for (key, maybe_value) in vars {
        if let Some(value) = maybe_value {
            env.insert(*key, value);
        }
    }

    let mut args: Vec<OsString> = vec![OsString::from("frankie")];
    args.extend(cli_args.iter().map(OsString::from));

    FrankieConfig::load_from_iter_with_env(args, Arc::new(env))
}

/// Applies a configuration layer to the composer based on the layer type.
pub fn apply_layer(composer: &mut MergeComposer, layer_type: &str, value: Value) {
    match layer_type {
        "defaults" => composer.push_defaults(value),
        "file" => composer.push_file(value, None),
        "environment" => composer.push_environment(value),
        "cli" => composer.push_cli(value),
        _ => panic!("unknown layer type: {layer_type}"),
    }
}

/// Helper to compose a [`FrankieConfig`] from a sequence of `(layer_type, value)` pairs.
///
/// # Errors
///
/// Returns the merge error when the layers do not compose into a valid
/// configuration; the calling test decides whether that is a failure.
pub fn build_config_from_layers(
    layers: &[(&str, Value)],
) -> Result<FrankieConfig, Arc<OrthoError>> {
    let mut composer = MergeComposer::new();

    for (layer_type, value) in layers {
        apply_layer(&mut composer, layer_type, value.clone());
    }

    FrankieConfig::merge_from_layers(composer.layers())
}
