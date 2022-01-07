// Copyright 2021 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::error::Error as StdError;
use std::path::Path;

use envoy_control_plane::envoy::config::bootstrap::v3::Bootstrap as V3Bootstrap;

use crate::util::file;

pub async fn load_config(path: &Path) -> Result<V3Bootstrap, Box<dyn StdError>> {
    let config_contents = file::read_all_utf8(path).await?;
    let config_ext = path.extension().unwrap_or_default();
    let bootstrap = if config_ext == "yaml" || config_ext == "yml" {
        eprintln!(
            "WARNING: YAML support is currently flakey (e.g. durations don't work) - use JSON"
        );
        serde_yaml::from_str(&config_contents)?
    } else {
        serde_json::from_str(&config_contents)?
    };
    Ok(bootstrap)
}
