// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

use astarte_device_sdk::EventLoop;
use clap::Parser;
use color_eyre::eyre;
use color_eyre::eyre::WrapErr;
use std::time::SystemTime;
use stream_rust_test::astarte::{send_data, ConnectionConfigBuilder, SdkConnection};
use stream_rust_test::cli::Config;
use stream_rust_test::shutdown::shutdown;
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

    // time instant when the program starts its execution
    let now = SystemTime::now();

    // initialize CLI configuration options
    let cli_cfg = Config::parse();

    debug!("parsed CLI config: {:#?}", cli_cfg);

    let mut tasks = JoinSet::<eyre::Result<()>>::new();

    // populate the builder using the environment variables (if set)
    let mut astarte_cfg_builder = ConnectionConfigBuilder::default();

    info!("retrieve Astarte connection config from ENV");
    if let Err(err) = astarte_cfg_builder.try_from_env() {
        warn!("failed to retrieve Astarte connection config from ENV: {err}");

        if let Some(path) = &cli_cfg.astarte_config_path {
            let path = path.join("config.toml");
            info!("retrieve Astarte connection config from {}", path.display());

            astarte_cfg_builder.from_toml(path).await;
        }
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

    // spawn task to send data to Astarte
    tasks.spawn(send_data(client, now, cli_cfg));

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
