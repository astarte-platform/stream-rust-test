// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

use astarte_device_sdk::EventLoop;
use clap::Parser;
use color_eyre::eyre;
use std::env;
use std::env::VarError;
use std::time::SystemTime;
use stream_rust_test::astarte::{send_data, ConnectionConfigBuilder, SdkConnection};
use stream_rust_test::cli::Config;
use tokio::task::JoinSet;
use tracing::{debug, error};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

fn env_filter() -> eyre::Result<EnvFilter> {
    let filter = env::var("RUST_LOG").or_else(|err| match err {
        VarError::NotPresent => Ok("stream_rust_test=debug".to_string()),
        err @ VarError::NotUnicode(_) => Err(err),
    })?;

    let env_filter = EnvFilter::try_new(filter)?;

    Ok(env_filter)
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

    debug!("Parsed config: {:#?}", cli_cfg);

    let mut tasks = JoinSet::<eyre::Result<()>>::new();

    // Load astarte configuration
    let mut astarte_cfg_builder =
        ConnectionConfigBuilder::with_connection(cli_cfg.astarte_connection);

    // populate the builder using the environment variables (if set)
    astarte_cfg_builder.from_env();

    if let Some(path) = &cli_cfg.astarte_config_path {
        let path = path.join("config.toml");
        astarte_cfg_builder.update_with_toml(path).await;
    }

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
    while let Some(res) = tasks.join_next().await {
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

    Ok(())
}
