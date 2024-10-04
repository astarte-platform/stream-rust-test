// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! Astarte builder and configuration structures.

use crate::cli::Config;
use crate::math::BaseValue;
use astarte_device_sdk::builder::{DeviceBuilder, DeviceSdkBuild};
use astarte_device_sdk::store::SqliteStore;
use astarte_device_sdk::transport::grpc::{Grpc, GrpcConfig};
use astarte_device_sdk::transport::mqtt::{Credential, Mqtt, MqttConfig};
use astarte_device_sdk::{Client, DeviceClient, DeviceConnection};
use clap::ValueEnum;
use color_eyre::eyre;
use color_eyre::eyre::{ContextCompat, OptionExt, WrapErr};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use std::{env, io};
use tracing::{debug, error, info};

/// Stream Rust test node identifier
const STREAM_RUST_TEST_NODE_UUID: uuid::Uuid = uuid::uuid!("d72a6187-7cf1-44cc-87e8-e991936166dc");

const DEVICE_DATASTREAM: &str =
    include_str!("../interfaces/org.astarte-platform.genericsensors.Values.json");

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
    mqtt_config: MqttConfigBuilder,
    /// Astarte Message Hub config options
    #[serde(rename = "grpc", default)]
    grpc_config: GrpcConfigBuilder,
}

impl ConnectionConfigBuilder {
    /// Builder constructor
    ///
    /// Specify if the builder should use the Astarte Device SDK or the Astarte Message Hub
    pub fn with_connection(astarte_connection: Option<AstarteConnection>) -> Self {
        Self {
            astarte_connection,
            store_directory: None,
            mqtt_config: MqttConfigBuilder::default(),
            grpc_config: GrpcConfigBuilder::default(),
        }
    }

    /// Init astarte config from env var if they have been set
    pub fn from_env(&mut self) {
        // doesn't change it if it's been set from CLI
        if self.astarte_connection.is_none() {
            self.astarte_connection = env::var("ASTARTE_CONNECTION")
                .ok()
                .map(|s| AstarteConnection::from_str(&s, true))
                .transpose()
                .ok()
                .unwrap_or_default();
        }

        self.store_directory = env::var("ASTARTE_STORE_DIRECTORY").ok().map(PathBuf::from);

        // update the mqtt config info
        let device_id = env::var("ASTARTE_DEVICE_ID").ok();
        let realm = env::var("ASTARTE_REALM").ok();
        let pairing_url = env::var("ASTARTE_PAIRING_URL").ok();
        let astarte_ignore_ssl = env::var("ASTARTE_IGNORE_SSL_ERRORS")
            .map(|s| s.parse().unwrap_or_default())
            .ok();
        let credential = env::var("ASTARTE_CREDENTIALS_SECRET")
            .ok()
            .map(Credential::secret)
            .or_else(|| {
                env::var("ASTARTE_PAIRING_TOKEN")
                    .ok()
                    .map(Credential::paring_token)
            });

        self.mqtt_config = MqttConfigBuilder {
            device_id,
            realm,
            credential,
            pairing_url,
            astarte_ignore_ssl,
        };

        // update the mqtt config info
        let endpoint = env::var("ASTARTE_MSGHUB_ENDPOINT").ok();

        self.grpc_config = GrpcConfigBuilder { endpoint };
    }

    /// Update the missing config values taking them from a config.toml file
    pub async fn update_with_toml(&mut self, path: impl AsRef<Path>) {
        match tokio::fs::read_to_string(&path).await {
            Ok(file) => {
                // retrieve the astarte config information from the config.toml file
                match toml::from_str::<ConfigToml>(&file) {
                    Ok(toml_cfg) => {
                        // update the configs
                        self.merge(toml_cfg.astarte);
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

    /// Merge two configs
    ///
    /// Prioritize the already existing fields
    fn merge(&mut self, other: ConnectionConfigBuilder) {
        // doesn't change it if it's been set from CLI or from ENV
        if self.astarte_connection.is_none() {
            self.astarte_connection = other.astarte_connection;
        }

        self.store_directory = self.store_directory.take().or(other.store_directory);

        // update the mqtt config info
        let mqtt_config = &mut self.mqtt_config;

        mqtt_config.device_id = mqtt_config.device_id.take().or(other.mqtt_config.device_id);
        mqtt_config.realm = mqtt_config.realm.take().or(other.mqtt_config.realm);
        mqtt_config.credential = mqtt_config
            .credential
            .take()
            .or(other.mqtt_config.credential);
        mqtt_config.pairing_url = mqtt_config
            .pairing_url
            .take()
            .or(other.mqtt_config.pairing_url);
        mqtt_config.astarte_ignore_ssl = mqtt_config
            .astarte_ignore_ssl
            .take()
            .or(other.mqtt_config.astarte_ignore_ssl);

        // update the grpc config info
        let grpc_config = &mut self.grpc_config;

        grpc_config.endpoint = grpc_config.endpoint.take().or(other.grpc_config.endpoint);
    }

    /// Build a complete Astarte configuration or return an error
    pub async fn build(self) -> eyre::Result<(DeviceClient<SqliteStore>, SdkConnection)> {
        let astarte_connection = self
            .astarte_connection
            .ok_or_eyre("missing astarte connection")?;

        let store_directory = self.store_directory.ok_or_eyre("missing store directory")?;

        let store = connect_store(&store_directory).await?;

        match astarte_connection {
            AstarteConnection::Mqtt => {
                let astarte_cfg = self.mqtt_config.build()?;
                debug!("parsed Astarte Device Sdk config: {:#?}", astarte_cfg);

                // define MQTT configuration options
                let mut mqtt_config = MqttConfig::new(
                    astarte_cfg.realm,
                    astarte_cfg.device_id,
                    astarte_cfg.credential,
                    astarte_cfg.pairing_url,
                );

                if astarte_cfg.astarte_ignore_ssl {
                    mqtt_config.ignore_ssl_errors();
                }

                // connect to Astarte
                let (client, connection) = DeviceBuilder::new()
                    .store(store)
                    .interface_str(DEVICE_DATASTREAM)?
                    .connect(mqtt_config)
                    .await?
                    .build()
                    .await;

                Ok((client, SdkConnection::Mqtt(connection)))
            }
            AstarteConnection::Grpc => {
                let astarte_cfg = self.grpc_config.build()?;
                debug!("parsed Astarte Message Hub config: {:#?}", astarte_cfg);

                let grpc_cfg =
                    GrpcConfig::from_url(STREAM_RUST_TEST_NODE_UUID, astarte_cfg.endpoint)
                        .wrap_err("failed to create a gRPC config")?;

                let (client, connection) = DeviceBuilder::new()
                    .store(store)
                    .interface_str(DEVICE_DATASTREAM)?
                    .connect(grpc_cfg)
                    .await?
                    .build()
                    .await;

                Ok((client, SdkConnection::Grpc(connection)))
            }
        }
    }
}

/// SDK [`DeviceConnection`]
pub enum SdkConnection {
    /// Mqtt [DeviceConnection]
    Mqtt(DeviceConnection<SqliteStore, Mqtt<SqliteStore>>),
    /// Grpc [DeviceConnection]
    Grpc(DeviceConnection<SqliteStore, Grpc<SqliteStore>>),
}

/// Config for an MQTT connection to Astarte
#[derive(Debug, Default, Deserialize)]
struct MqttConfigBuilder {
    /// Device ID
    device_id: Option<String>,
    /// Astarte realm
    realm: Option<String>,
    /// Device credential
    #[serde(flatten)]
    credential: Option<Credential>,
    /// Astarte pairing url
    pairing_url: Option<String>,
    /// Flag to ignore Astarte SSL errors
    astarte_ignore_ssl: Option<bool>,
}

impl MqttConfigBuilder {
    fn build(self) -> eyre::Result<MqttConfigChecked> {
        Ok(MqttConfigChecked {
            device_id: self.device_id.ok_or_eyre("missing device id")?,
            realm: self.realm.ok_or_eyre("missing realm")?,
            credential: self
                .credential
                .ok_or_eyre("missing either a credential secret or a pairing token")?,
            pairing_url: self.pairing_url.ok_or_eyre("missing pairing url")?,
            // if missing, set the ignore ssl error flat to false
            astarte_ignore_ssl: self.astarte_ignore_ssl.unwrap_or_default(),
        })
    }
}

/// Config for a gRPC connection to an Astarte Message Hub instance
#[derive(Debug, Default, Deserialize)]
struct GrpcConfigBuilder {
    /// The Endpoint of the Astarte Message Hub
    endpoint: Option<String>,
}

impl GrpcConfigBuilder {
    fn build(self) -> eyre::Result<GrpcConfigChecked> {
        Ok(GrpcConfigChecked {
            endpoint: self.endpoint.ok_or_eyre("missing endpoint")?,
        })
    }
}

/// Astarte device configuration
#[derive(Debug)]
struct MqttConfigChecked {
    /// Astarte realm
    realm: String,
    /// Device ID
    device_id: String,
    /// Device credential
    credential: Credential,
    /// Astarte pairing url
    pairing_url: String,
    /// Flag to ignore Astarte SSL errors
    astarte_ignore_ssl: bool,
}

/// Astarte device configuration
#[derive(Debug)]
struct GrpcConfigChecked {
    /// The Endpoint of the Astarte Message Hub
    endpoint: String,
}

/// Connect to the store.
pub async fn connect_store<P>(store_dir: P) -> eyre::Result<SqliteStore>
where
    P: AsRef<Path>,
{
    let db_path = store_dir.as_ref().join("database.db");
    let db_path_str = db_path.to_str().wrap_err("wrong store db path")?;

    debug!("connecting to store {db_path_str}");
    let store = SqliteStore::connect_db(db_path_str)
        .await
        .wrap_err("failed to connect to store")?;

    info!("connected to store {db_path_str}");
    Ok(store)
}

/// Send data to Astarte
pub async fn send_data(
    client: DeviceClient<SqliteStore>,
    now: SystemTime,
    cfg: Config,
) -> eyre::Result<()> {
    let mut base_value = BaseValue::try_from_system_time(now, cfg.scale)?;

    debug!(
        "sending data to Astarte with {} math function",
        cfg.math_function
    );

    loop {
        // Send data to Astarte
        let value = cfg.math_function.compute(base_value.value());

        client
            .send(&cfg.interface_datastream_do, "/test/value", value)
            .await?;

        debug!("data sent on endpoint /test/value, content: {value}");

        // update the data to send at the next iteration
        base_value.update();

        // Sleep interval secs
        tokio::time::sleep(std::time::Duration::from_millis(cfg.interval_btw_samples)).await;
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_connection_config_builder_build_failures() {
        // empty config builder cannot build successfully
        let cfg_builder = ConnectionConfigBuilder::with_connection(None).build().await;
        assert!(cfg_builder.is_err());

        // config builder with only the connection to astarte specified cannot build successfully
        let con = AstarteConnection::Mqtt;
        let cfg_builder = ConnectionConfigBuilder::with_connection(Some(con))
            .build()
            .await;
        assert!(cfg_builder.is_err());

        // check that only the astarte connection is added to the configuration
        let mut cfg_builder = ConnectionConfigBuilder::with_connection(None);
        let toml_str = r#"
        [astarte]
        connection = "mqtt"
        "#;
        let toml = toml::from_str::<ConfigToml>(toml_str).unwrap();
        cfg_builder.merge(toml.astarte);
        assert!(cfg_builder.astarte_connection.is_some());

        // check that the astarte connection is not updated with toml info if it already contains a
        // value
        let con = AstarteConnection::Mqtt;
        let mut cfg_builder = ConnectionConfigBuilder::with_connection(Some(con));
        let toml_str = r#"
        [astarte]
        connection = "grpc"
        "#;
        let toml = toml::from_str::<ConfigToml>(toml_str).unwrap();
        cfg_builder.merge(toml.astarte);
        assert_eq!(cfg_builder.astarte_connection, Some(con));

        // define store dire for the next tests
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

        // check that the store path is not updated with toml info if it already contains a value
        let mut cfg_builder = ConnectionConfigBuilder {
            astarte_connection: Some(AstarteConnection::Mqtt),
            store_directory: Some(tmp_dir.clone()),
            mqtt_config: Default::default(),
            grpc_config: Default::default(),
        };
        let toml_str = r#"
        [astarte]
        connection = "grpc"
        store_directory = "/tmp/stream-rust-test/store/"
        "#;
        let toml = toml::from_str::<ConfigToml>(toml_str).unwrap();
        cfg_builder.merge(toml.astarte);
        assert_eq!(cfg_builder.store_directory, Some(tmp_dir.clone()));
    }
}
