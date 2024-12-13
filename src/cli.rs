// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! CLI configuration options

use crate::math::MathFunction;
use clap::Parser;

/// Configuration for the values to be sent to Astarte
#[derive(Debug, Clone, Parser)]
#[clap(version, about)]
pub struct Config {
    #[cfg(not(feature = "docker"))]
    /// Path to the directory containing the Astarte configuration file config.toml
    ///
    /// First, the Astarte configuration is taken from ENV vars, then from the config.toml if the
    /// path has been specified
    #[clap(short, long, env = "ASTARTE_CONFIG_PATH")]
    pub astarte_config_path: Option<std::path::PathBuf>,
    /// Math function the device will use to send data to Astarte
    #[clap(short, long, default_value = "default", env = "MATH_FUNCTION")]
    pub math_function: MathFunction,
    /// Interface name to send data to
    #[clap(
        long,
        default_value = "org.astarte-platform.genericsensors.Values",
        env = "INTERFACE_NAME"
    )]
    pub interface_datastream_do: String,
    /// Milliseconds the device must wait before sending data to Astarte
    #[clap(short, long, default_value = "1000", env = "INTERVAL_BTW_SAMPLES")]
    pub interval_btw_samples: u64,
    /// Scale for the generation of the data to send
    #[clap(short, long, default_value = "1.0", env = "SCALE")]
    pub scale: f64,
}
