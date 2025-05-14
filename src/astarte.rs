// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! Astarte builder and configuration structures.

use astarte_device_sdk::builder::{DeviceBuilder, DeviceSdkBuild};
use astarte_device_sdk::store::SqliteStore;
use astarte_device_sdk::transport::grpc::{Grpc, GrpcConfig};
use astarte_device_sdk::transport::mqtt::{Credential, Mqtt, MqttConfig};
use astarte_device_sdk::{DeviceClient, DeviceConnection};
use clap::ValueEnum;
use color_eyre::eyre;
use color_eyre::eyre::{bail, eyre, OptionExt, WrapErr};
use serde::Deserialize;
use std::env::VarError;
use std::path::{Path, PathBuf};
use std::{env, io};
use tracing::{debug, error, instrument};
use uuid::{uuid, Uuid};

const DEVICE_DATASTREAM: &str =
    include_str!("../interfaces/org.astarte-platform.genericsensors.Values.json");

const SERVER_DATASTREAM: &str =
    include_str!("../interfaces/org.astarte-platform.genericcommands.ServerCommands.json");

const DEFAULT_STREAM_NODE_ID: Uuid = uuid!("d72a6187-7cf1-44cc-87e8-e991936166dc");

/// This function is necessary for serde deserialization
fn default_stream_node_id() -> Uuid {
    DEFAULT_STREAM_NODE_ID
}

/// Specify which Astarte library use to connect to Astarte
#[derive(
    Debug, Clone, Copy, Default, Eq, PartialEq, Ord, PartialOrd, clap::ValueEnum, Deserialize,
)]
pub enum AstarteConnection {
    /// Connect through MQTT to Astarte
    #[default]
    #[serde(rename = "mqtt")]
    Mqtt,
    /// Connect through gRPC to an Astarte Message Hub instance already connected to Astarte
    #[serde(rename = "grpc")]
    Grpc,
}

/// Astarte connection config toml
///
/// struct used to deserialize the TOML file
#[derive(Debug, Default, Deserialize)]
struct ConfigToml {
    astarte: ConnectionConfigBuilder,
}

/// Astarte connection config Builder
#[derive(Debug, Default, Deserialize)]
pub struct ConnectionConfigBuilder {
    /// Astarte connection
    #[serde(rename = "connection")]
    astarte_connection: Option<AstarteConnection>,
    /// Astarte store directory
    store_directory: Option<PathBuf>,
    /// Astarte Device SDK config options
    #[serde(rename = "mqtt", default)]
    mqtt_config: Option<MqttConfigBuilder>,
    /// Astarte Message Hub endpoint
    #[serde(rename = "grpc", default)]
    grpc_config: Option<GrpcConfigBuilder>,
}

impl ConnectionConfigBuilder {
    /// Init astarte config from env var if they have been set
    ///
    /// If an error is returned, it means that one or more environment variables have not been set
    #[instrument(skip_all)]
    pub fn try_from_env(&mut self) -> eyre::Result<()> {
        let con = env::var("ASTARTE_CONNECTION")
            .map(|s| AstarteConnection::from_str(&s, true))?
            .map_err(|err| eyre!(err))?;

        self.store_directory = Some(env::var("ASTARTE_STORE_DIRECTORY").map(PathBuf::from)?);

        match con {
            AstarteConnection::Mqtt => {
                self.astarte_connection = Some(con);

                // update the mqtt config info
                let device_id = env::var("ASTARTE_DEVICE_ID")?;
                let realm = env::var("ASTARTE_REALM")?;
                let pairing_url = env::var("ASTARTE_PAIRING_URL")?;
                let astarte_ignore_ssl = env::var("ASTARTE_IGNORE_SSL_ERRORS")
                    .map(|s| s.parse::<bool>().unwrap_or_default())?;
                let credential = env::var("ASTARTE_CREDENTIALS_SECRET")
                    .map(Credential::secret)
                    .or_else(|_| env::var("ASTARTE_PAIRING_TOKEN").map(Credential::paring_token))?;

                self.mqtt_config = Some(MqttConfigBuilder {
                    device_id,
                    realm,
                    credential,
                    pairing_url,
                    ignore_ssl_errors: astarte_ignore_ssl,
                });
            }
            AstarteConnection::Grpc => {
                self.astarte_connection = Some(con);

                let endpoint = env::var("ASTARTE_MSGHUB_ENDPOINT")?;

                let node_id = match env::var("ASTARTE_MSGHUB_NODE_ID") {
                    Ok(uuid) => Uuid::parse_str(&uuid)
                        .wrap_err(format!("invalid ASTARTE_MSGHUB_NODE_ID {uuid}"))?,
                    Err(VarError::NotPresent) => DEFAULT_STREAM_NODE_ID,
                    Err(VarError::NotUnicode(s)) => {
                        bail!("non unicode ASTARTE_MSGHUB_NODE_ID {s:?}")
                    }
                };

                // update the grpc config info
                self.grpc_config = Some(GrpcConfigBuilder { node_id, endpoint });
            }
        }

        Ok(())
    }

    /// Update the missing config values taking them from a config.toml file
    #[instrument(skip_all)]
    pub async fn from_toml(&mut self, path: impl AsRef<Path>) {
        match tokio::fs::read_to_string(&path).await {
            Ok(file) => {
                // retrieve the astarte config information from the config.toml file
                match toml::from_str::<ConfigToml>(&file) {
                    Ok(toml_cfg) => {
                        // update the configs
                        *self = toml_cfg.astarte;
                    }
                    Err(err) => {
                        error!("error deserializing astarte cfg from toml: {err}");
                    }
                }
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                error!("file {} not found", path.as_ref().display());
            }
            Err(err) => {
                error!("error trying to read {}: {err}", path.as_ref().display());
            }
        }
    }

    /// Build a complete Astarte configuration or return an error
    #[instrument(skip_all)]
    pub async fn build(self) -> eyre::Result<(DeviceClient<SqliteStore>, SdkConnection)> {
        let astarte_connection = self
            .astarte_connection
            .ok_or_eyre("missing astarte connection")?;

        let store_directory = self.store_directory.ok_or_eyre("missing store directory")?;

        let builder = DeviceBuilder::new()
            .store_dir(&store_directory)
            .await?
            .interface_str(DEVICE_DATASTREAM)?
            .interface_str(SERVER_DATASTREAM)?;

        match astarte_connection {
            AstarteConnection::Mqtt => {
                // define MQTT configuration options
                let mqtt_cfg: MqttConfig =
                    self.mqtt_config.ok_or_eyre("invalid mqtt config")?.into();
                debug!("parsed Astarte Device Sdk config: {:#?}", mqtt_cfg);

                // connect to Astarte
                let (client, connection) = builder.connect(mqtt_cfg).await?.build().await;

                Ok((client, SdkConnection::Mqtt(Box::new(connection))))
            }
            AstarteConnection::Grpc => {
                let grpc_cfg = self
                    .grpc_config
                    .ok_or_eyre("invalid grpc config")?
                    .build()?;

                debug!("parsed Astarte Message Hub config: {:#?}", grpc_cfg);

                let (client, connection) = builder.connect(grpc_cfg).await?.build().await;

                Ok((client, SdkConnection::Grpc(Box::new(connection))))
            }
        }
    }
}

/// SDK [`DeviceConnection`]
pub enum SdkConnection {
    /// Mqtt [DeviceConnection]
    Mqtt(Box<DeviceConnection<SqliteStore, Mqtt<SqliteStore>>>),
    /// Grpc [DeviceConnection]
    Grpc(Box<DeviceConnection<SqliteStore, Grpc<SqliteStore>>>),
}

/// Config for an MQTT connection to Astarte
///
/// The struct isn't really necessary, nevertheless we cannot deserialize the entire [MqttConfig]
/// struct without having the fields `keepalive`, `conn_timeout` and `bounded_channel_size`.
#[derive(Debug, Deserialize)]
struct MqttConfigBuilder {
    /// Device ID
    device_id: String,
    /// Astarte realm
    realm: String,
    /// Device credential
    #[serde(flatten)]
    credential: Credential,
    /// Astarte pairing url
    pairing_url: String,
    /// Flag to ignore Astarte SSL errors
    ignore_ssl_errors: bool,
}

impl From<MqttConfigBuilder> for MqttConfig {
    fn from(value: MqttConfigBuilder) -> Self {
        let mut cfg = MqttConfig::new(
            value.realm,
            value.device_id,
            value.credential,
            value.pairing_url,
        );

        if value.ignore_ssl_errors {
            cfg.ignore_ssl_errors();
        }

        cfg
    }
}

/// Config for a gRPC connection to an Astarte Message Hub instance
#[derive(Debug, Default, Deserialize)]
struct GrpcConfigBuilder {
    #[serde(default = "default_stream_node_id")]
    /// Stream Rust test UUID
    node_id: Uuid,
    /// The Endpoint of the Astarte Message Hub
    endpoint: String,
}

impl GrpcConfigBuilder {
    fn build(self) -> eyre::Result<GrpcConfig> {
        GrpcConfig::from_url(self.node_id, self.endpoint).wrap_err("failed to create a gRPC config")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_connection_config_builder_build_failures() {
        // empty config builder cannot build successfully
        let cfg_builder = ConnectionConfigBuilder::default().build().await;
        assert!(cfg_builder.is_err());

        // define store dir for the next tests
        let mut tmp_dir = env::temp_dir();
        tmp_dir.push("stream-rust-test-tests");
        std::fs::create_dir_all(&tmp_dir).expect("failed to create store dir");

        // cannot build successfully only with astarte connection and store path information
        let cfg_builder = ConnectionConfigBuilder {
            astarte_connection: Some(AstarteConnection::Mqtt),
            store_directory: Some(tmp_dir.clone()),
            mqtt_config: Default::default(),
            grpc_config: Default::default(),
        };
        let res = cfg_builder.build().await;
        assert!(res.is_err());
    }
}
