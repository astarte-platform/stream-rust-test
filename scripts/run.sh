#!/bin/bash

# This file is part of Astarte.
#
# Copyright 2024 SECO Mind Srl
#
# SPDX-License-Identifier: Apache-2.0

set -eEuo pipefail

# Set application environment variables
export MATH_FUNCTION="sin"
export INTERVAL_BTW_SAMPLES=500
export SCALE=3

# Navigate to the project root
cd "$(dirname "$0")/.."

# Run the Rust application
# Assuming your compiled binary is located in the `target/debug/` directory
./target/debug/stream-rust-test
