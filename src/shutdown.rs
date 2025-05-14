// This file is part of Astarte.
//
// Copyright 2024-2025 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! Define shutdown futures to stop the docker container with CTRL+C command

use color_eyre::eyre;
use tracing::{error, instrument};

#[cfg(unix)]
/// Shut down the application in case a SIGTERM or SIGINT is received.
#[instrument(skip_all)]
pub fn shutdown() -> eyre::Result<impl std::future::Future<Output = ()>> {
    use color_eyre::eyre::WrapErr;
    use futures::FutureExt;
    use tokio::signal::unix::SignalKind;

    let mut term = tokio::signal::unix::signal(SignalKind::terminate())
        .wrap_err("couldn't create SIGTERM listener")?;

    let future = async move {
        let term = std::pin::pin!(async move {
            if term.recv().await.is_none() {
                error!("no more signal events can be received")
            }
        });

        let ctrl_c = std::pin::pin!(tokio::signal::ctrl_c().map(|res| {
            if let Err(err) = res {
                error!("couldn't receive SIGINT {err}");
            }
        }));

        futures::future::select(term, ctrl_c).await;
    };

    Ok(future)
}

#[cfg(not(unix))]
/// Shut down the application in case a SIGINT is received.
#[instrument(skip_all)]
pub fn shutdown() -> eyre::Result<impl std::future::Future<Output = ()>> {
    use futures::FutureExt;

    Ok(tokio::signal::ctrl_c().map(|res| {
        if let Err(err) = res {
            error!("couldn't receive SIGINT {err}");
        }
    }))
}
