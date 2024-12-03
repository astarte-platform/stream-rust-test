<!--
Copyright 2024 SECO Mind Srl

SPDX-License-Identifier: Apache-2.0
-->

# Astarte Stream Rust Test

[![ci](https://github.com/astarte-platform/stream-rust-test/actions/workflows/ci.yaml/badge.svg)](https://github.com/astarte-platform/stream-rust-test/actions/workflows/ci.yaml)
[![codecov](https://codecov.io/gh/astarte-platform/stream-rust-test/graph/badge.svg?token=wW2Hsm5edX)](https://codecov.io/gh/astarte-platform/stream-rust-test)

Astarte Rust SDK based data stream test.

## Requirements

* Astarte Device Rust SDK

## Getting started

### Choosing a Device ID

A Base64 url-encoded uuid should be used, you can use [astartectl](https://github.com/astarte-platform/astartectl#installation) to generate one:

```bash
astartectl utils device-id generate-random
```

## Configuring the application

You can configure the application either by using a direct MQTT connection to an Astarte instance or by connecting it
through gRPC to an Astarte Message Hub instance (already configured and connected to Astarte).

The application can be configured by either using environment variables or a `config.toml` file.

If you want to use environment variables to set up the application, you can set the following:
- `ASTARTE_CONNECTION`: which type of connection to Astarte must be used
- `ASTARTE_REALM`: name of the astarte realm
- `ASTARTE_DEVICE_ID`: astarte device id
- `ASTARTE_CREDENTIALS_SECRET`: astarte device credential secret
- `ASTARTE_PAIRING_TOKEN`: token used to register a device and obtain a `credentials_secret`
- `ASTARTE_PAIRING_URL`: address of the astarte broker to connect the device to Astarte
- `ASTARTE_STORE_DIRECTORY`: path to the directory where to store data (e.g., in case of Astarte properties)
- `ASTARTE_IGNORE_SSL_ERRORS`: boolean stating if SSL errors should be ignored (default: false)
- `ASTARTE_MSGHUB_ENDPOINT`: endpoint of the Astarte Message Hub instance
- `ASTARTE_MSGHUB_NODE_ID`: UUID of the Node to connect to the Astarte Message Hub

Instead, if you want to use a configuration file, you must specify its location by using the `ASTARTE_CONFIG_PATH`
environment variable. The `config.toml` file must contain the following information:
```toml
[astarte]
connection = "mqtt" # or "grpc"
store_directory = "/tmp/stream-rust-test/store/"

# MQTT connection to Astarte
[astarte.mqtt]
realm = "REALM_NAME_HERE"
device_id = "DEVICE_ID_HERE"
pairing_url = "PAIRING_URL_HERE"
credentials_secret = "CREDENTIALS_SECRET_HERE"
#pairing_token = "PAIRING_TOKEN_HERE"
astarte_ignore_ssl = false

# gRPC connection to the Astarte Message Hub
[astarte.grpc]
endpoint = "http://[::1]:50051"
#node_id = "ASTARTE_MSGHUB_NODE_ID_HERE"
```

NOTE: only one of the `[astarte.mqtt]` or `[astarte.grpc]` sections should be specified in the file.

A detailed description of the fields is depicted below:
- `connection`: a field indicating which type of connection to Astarte should be used.
- `store_directory`: the directory specifying where persistent data will be saved.
- `realm`: the name of the Astarte realm.
- `device_id`: the id of the device you want to connect to Astarte.
- `pairing_url`: the URL of the Astarte Pairing endpoint (e.g., `https://<api url>/pairing`.)
- `credentials_secret` or `pairing_token`: the identifiers used to authenticate the device through Astarte. If both are
  present, the credential secret will be used.
- `astarte_ignore_ssl`: a flag stating if SSL errors should be ignored when connecting to Astarte.
- `endpoint`: the endpoint where the Astarte Message Hub instance is listening for new connections.
- `node_id`: UUID of the Node to connect to the Astarte Message Hub (optional).

Since the application can be configured with a CLI, when [running the application](#build-and-run) you can specify the
type of connection (`mqtt` or `grpc`) and the path to the `config.toml` file with the `--astarte_connection` (`-c`) and
`--astarte_config_path` options, respectively.

### Build and run

Build the application using following commands:
```sh
cargo build --release
```

Then run the application either by running the `run.sh` script inside the `scripts` folder or with the CLI:
```sh
./target/release/stream-rust-test [OPTIONS]
```

The full list of options can be shown with the command:
```sh
./target/release/stream-rust-test -h
```

Or you can use the `--help` option (instead of `-h`) to have a more detailed explanation of the available options.

The following options can be set:
- `--device`: set the device ID;
- `--function`: choose the data generation function (one between `sin`, `noisesin`, `randomspikessin`, `saw`,
  `rect`, `sinc`, `random`, `x` and a default one);
- `--interval`: set the sending interval;
- `--scale`: set the scale of the generated result;

You can also set the stream options by using the following environment variables:
- `MATH_FUNCTION`
- `INTERFACE_NAME`
- `INTERVAL_BTW_SAMPLES`
- `SCALE`

## Docker

### Build the Container

First, ensure you have Docker installed on your machine, then:
1. Clone the repository containing the Stream Rust Test code
2. Navigate to the root directory of the repository
3. Run the build script to create the Docker image:
   ```sh
   ./scripts/docker/build.sh
   ```

### Run the Container

#### Run the container with your configuration file

To run the container with your configuration file:
1. Ensure you have defined your astarte configuration file `config.toml`
2. Run the Docker container, mounting the configuration file:
   ```sh
   docker run -v /path/to/your/config.toml:<MOUNT_TO_THIS_PATH> -e ASTARTE_CONFIG_PATH="<MOUNT_TO_THIS_PATH>" stream-rust-test:latest
   ```

Replace `/path/to/your/config.toml` with the actual path to your configuration file.

Note: `MOUNT_TO_THIS_PATH` must be an absolute path.

#### Run the container with environment variables

You can configure the application with environment variables by exporting them (e.g. configuring
them in the [docker-compose.yaml](https://docs.docker.com/compose/environment-variables/set-environment-variables/))
or via the `--env-file` CLI options:

```sh
docker run --env-file /path/to/your/.env stream-rust-test:latest
```

Consult the `--help` for a full list of environment variable names and options.

### Run the container in a separate network

If you are running a local astarte instance, such the one in
[Astarte in 5 minutes](https://docs.astarte-platform.org/astarte/latest/010-astarte_in_5_minutes.html),
you can either modify the `docker-compose.yaml` by adding the `stream-rust-test` container to it or you can run it
by using the `--network="host"` parameter, which is required to make `localhost` work.

```sh
docker run --network="host" [ENV VARS] [MOUNT config.toml] stream-rust-test:latest
```
