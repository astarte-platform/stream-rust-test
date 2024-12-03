#!/usr/bin/env bash

# Copyright 2024 SECO Mind Srl
#
# SPDX-License-Identifier: Apache-2.0

set -exEuo pipefail

repo=$(git rev-parse --show-toplevel)

cd "$repo"

# Run the docker build context from the root of the repo
docker build -f scripts/docker/Dockerfile . -t stream-rust-test:latest
