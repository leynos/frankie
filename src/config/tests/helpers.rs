//! Shared test helpers for configuration tests.

use ortho_config::MergeComposer;
use serde_json::Value;

use crate::FrankieConfig;

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
pub fn build_config_from_layers(layers: &[(&str, Value)]) -> FrankieConfig {
    let mut composer = MergeComposer::new();

    for (layer_type, value) in layers {
        apply_layer(&mut composer, layer_type, value.clone());
    }

    FrankieConfig::merge_from_layers(composer.layers()).expect("merge should succeed")
}
