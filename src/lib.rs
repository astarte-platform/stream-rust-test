// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

#![doc = include_str!("../README.md")]
#![warn(clippy::dbg_macro, missing_docs, rustdoc::missing_crate_level_docs)]

use astarte_device_sdk::client::RecvError;
use astarte_device_sdk::store::SqliteStore;
use astarte_device_sdk::{Client, DeviceClient, DeviceEvent};
use color_eyre::eyre;
use color_eyre::eyre::OptionExt;
use std::time::SystemTime;
use tokio::select;
use tracing::{debug, error, instrument};

use crate::cli::Config;
use crate::config::{StreamConfig, StreamConfigUpdate};

pub mod astarte;
pub mod cli;
pub mod config;
pub mod math;
pub mod shutdown;

/// Stream manager
///
/// It handles the following operations:
/// - send stream data to Astarte
/// - receive new stream configuration from Astarte
pub struct StreamManager {
    stream_cfg: StreamConfig,
}

impl StreamManager {
    /// Build a [StreamManager] from an initial configuration
    pub async fn new(cfg: Config) -> eyre::Result<Self> {
        // time instant when the program starts its execution
        let now = SystemTime::now();
        let stream_cfg = StreamConfig::try_from_cli(cfg, now)?;

        Ok(Self { stream_cfg })
    }

    /// Handle sending data to Astarte and the reception of new stream configuration from Astarte
    #[instrument(skip_all)]
    pub async fn handle(mut self, client: DeviceClient<SqliteStore>) -> eyre::Result<()> {
        loop {
            select! {
                // send data after every timeout only if the stream is not paused
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(self.stream_cfg.interval)), if self.stream_cfg.is_on() => {
                    debug!(
                        "sending data to Astarte with {} math function and scale {}",
                        self.stream_cfg.math_function, self.stream_cfg.scale
                    );
                    self.send_data(&client).await?;
                }
                // receive data from Astarte
                res = client.recv() => {
                    match res {
                        Ok(event) => {
                            debug!("received event from Astarte: {:?}", event);
                            self.receive_data(event).await?;
                        }
                        Err(RecvError::Disconnected) => return Ok(()),
                        Err(err) => error!(%err),
                    }
                }
            }
        }
    }

    /// Send data to Astarte
    #[instrument(skip_all)]
    async fn send_data(&mut self, client: &DeviceClient<SqliteStore>) -> eyre::Result<()> {
        // Send data to Astarte
        let value = self.stream_cfg.next_value();

        // TODO: here the sensor_id is static. Should we introduce it as a CLI argument or another way to receive it "dynamically"?
        client
            .send(self.stream_cfg.interface(), "/sensor_id_123/value", value)
            .await?;

        debug!("data sent on endpoint /sensor_id_123/value, content: {value}");

        // update the value upon which the data to be sent to Astarte at the next iteration will be computed
        self.stream_cfg.update_value();

        Ok(())
    }

    /// Receive new stream configuration from Astarte.
    #[instrument(skip_all)]
    async fn receive_data(&mut self, event: DeviceEvent) -> eyre::Result<()> {
        if let astarte_device_sdk::Value::Individual(var) = event.data {
            // split the mapping path, which looks like "/foo/bar"
            let mut iter = event.path.splitn(3, '/').skip(1);

            let sensor_id = iter.next().ok_or_eyre("missing sensor id")?;

            match iter.next() {
                Some("toggle") => {
                    debug!("Received new toggle datastream for sensor {sensor_id}.");
                    self.stream_cfg
                        .update_cfg(StreamConfigUpdate::toggle_state(sensor_id))
                        .await;
                }
                Some("function") => {
                    let function = String::try_from(var)?.into();
                    debug!(
                        "Received new function datastream for sensor {sensor_id}. sensor function is now {function}"
                    );
                    self.stream_cfg
                        .update_cfg(StreamConfigUpdate::function(sensor_id, function))
                        .await;
                }
                Some("interval") => {
                    let interval = i64::try_from(var)?.try_into()?;
                    debug!(
                        "Received new interval datastream for sensor {sensor_id}. sensor interval is now {interval}"
                    );
                    self.stream_cfg
                        .update_cfg(StreamConfigUpdate::interval(sensor_id, interval))
                        .await;
                }
                Some("scale") => {
                    let scale = var.try_into()?;
                    debug!(
                        "Received new scale datastream for sensor {sensor_id}. sensor scale is now {scale}"
                    );
                    self.stream_cfg
                        .update_cfg(StreamConfigUpdate::scale(sensor_id, scale))
                        .await;
                }
                item => {
                    error!("unrecognized {item:?}")
                }
            }
        }

        Ok(())
    }
}
