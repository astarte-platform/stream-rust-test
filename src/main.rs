// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

use astarte_device_sdk::EventLoop;
use clap::Parser;
use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use stream_rust_test::astarte::{ConnectionConfigBuilder, SdkConnection};
use stream_rust_test::cli::Config;
use stream_rust_test::shutdown::shutdown;
use stream_rust_test::StreamManager;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

fn env_filter() -> eyre::Result<EnvFilter> {
    EnvFilter::builder()
        .with_default_directive("stream_rust_test=debug".parse()?)
        .from_env()
        .wrap_err("failed to set debug filter")
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let filter = env_filter()?;
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(filter)
        .init();

    // initialize CLI configuration options
    let cli_cfg = Config::parse();

    debug!("parsed CLI config: {:#?}", cli_cfg);

    let mut tasks = JoinSet::<eyre::Result<()>>::new();

    // populate the builder using the environment variables (if set)
    let mut astarte_cfg_builder = ConnectionConfigBuilder::default();

    info!("retrieve Astarte connection config from ENV");
    if let Err(err) = astarte_cfg_builder.try_from_env() {
        warn!("failed to retrieve Astarte connection config from ENV: {err}");

        // default path where to search for config.toml
        let path_str = "/etc/stream-rust-test".to_string();

        #[cfg(not(feature = "docker"))]
        // if we are not using containers, we try to retrieve the config file from a specific location
        // set by the user in the ASTARTE_CONFIG_PATH environment variable
        let path_str = match std::env::var("ASTARTE_CONFIG_PATH") {
            Ok(path) => {
                debug!("retrieve Astarte connection config from path: {path}");
                path
            }
            Err(std::env::VarError::NotPresent) => {
                debug!(
                    "retrieve Astarte connection config from default path (/etc/stream-rust-test)"
                );
                path_str
            }
            Err(err) => {
                error!("failed to retrieve Astarte connection config: {err}");
                return Err(color_eyre::Report::new(err));
            }
        };

        let path = std::path::PathBuf::from(path_str).join("config.toml");

        astarte_cfg_builder.from_toml(path).await;
    };

    let (client, connection) = astarte_cfg_builder.build().await?;

    match connection {
        SdkConnection::Mqtt(mqtt_con) => {
            // spawn task to poll updates from the MQTT connection with Astarte
            tasks.spawn(async move { mqtt_con.handle_events().await.map_err(Into::into) });
        }
        SdkConnection::Grpc(grpc_con) => {
            // spawn task to poll updates from the gRPC connection with the Astarte Message Hub
            tasks.spawn(async move { grpc_con.handle_events().await.map_err(Into::into) });
        }
    }

    let stream_manager = StreamManager::new(cli_cfg).await?;
    tasks.spawn(stream_manager.handle(client));

    // handle tasks termination
    loop {
        tokio::select! {
            _ = shutdown()? => {
                info!("CTRL C received, shutting down");
                tasks.abort_all();
                break;
            },
            opt = tasks.join_next() => {
                let Some(res) = opt else {
                    break;
                };

                match res {
                    Ok(Ok(())) => {}
                    Err(err) if err.is_cancelled() => {}
                    Err(err) => {
                        error!(error = %err, "Task panicked");
                        return Err(err.into());
                    }
                    Ok(Err(err)) => {
                        error!(error = %err, "Task returned an error");
                        return Err(err);
                    }
                }
            }
        }
    }

    Ok(())
}
