// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! Math functions used to simulate values sensored by the device to be sent to Astarte

use f64::consts::PI;
use std::f64;
use std::fmt::Display;

/// Math functions
#[derive(Debug, Clone, Default, clap::ValueEnum)]
pub enum MathFunction {
    /// Sine
    Sin,
    /// Noise sine
    NoiseSin,
    /// Random spike sine
    RandomSpikesSin,
    /// Constant value
    Const,
    /// Saw
    Saw,
    /// Rect
    Rect,
    /// Normalized Sinc
    ///
    /// sinc(x) = sin(PI*x) / (PI*x)
    Sinc,
    /// Random value
    Random,
    /// Default value
    #[default]
    Default,
}

impl Display for MathFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            MathFunction::Sin => "sin",
            MathFunction::NoiseSin => "noise sin",
            MathFunction::RandomSpikesSin => "random spikes sin",
            MathFunction::Const => "const",
            MathFunction::Saw => "saw",
            MathFunction::Rect => "rect",
            MathFunction::Sinc => "sinc",
            MathFunction::Random => "random",
            MathFunction::Default => "default",
        };

        write!(f, "{s}")
    }
}

impl<T: AsRef<str>> From<T> for MathFunction {
    fn from(value: T) -> Self {
        match value.as_ref() {
            "sin" => Self::Sin,
            "noise-sin" => Self::NoiseSin,
            "random-spikes-sin" => Self::RandomSpikesSin,
            "const" => Self::Const,
            "saw" => Self::Saw,
            "rect" => Self::Rect,
            "sinc" => Self::Sinc,
            "random" => Self::Random,
            _ => Self::Default,
        }
    }
}

impl MathFunction {
    /// Use math functions to compute an output value given a certain input
    pub fn compute(&self, value: f64) -> f64 {
        match self {
            MathFunction::Sin => sin(value),
            MathFunction::NoiseSin => noise_sin(value),
            MathFunction::RandomSpikesSin => random_spikes_sin(value),
            MathFunction::Const => constant(value),
            MathFunction::Saw => saw(value),
            MathFunction::Rect => rect(value),
            MathFunction::Sinc => sinc(value),
            MathFunction::Random => random(),
            MathFunction::Default => default(value),
        }
    }
}

fn sin(value: f64) -> f64 {
    value.sin()
}

fn noise_sin(value: f64) -> f64 {
    value.sin() + random()
}

fn random_spikes_sin(value: f64) -> f64 {
    let v = noise_sin(value);

    //  spike(x) = 1, if 0 ≤ x < 1, 0 elsewhere
    if (0.0..0.999).contains(&random()) {
        v
    } else {
        v + 100.0
    }
}

fn constant(value: f64) -> f64 {
    value
}

fn fmod(mut value: f64) -> f64 {
    value %= 2.0 * PI;
    value
}

fn saw(value: f64) -> f64 {
    (fmod(value) - PI) / PI
}

fn rect(value: f64) -> f64 {
    if fmod(value) - PI > 0.0 {
        1.0
    } else {
        0.0
    }
}

fn sinc(value: f64) -> f64 {
    let t = (value % (20.0 * PI)) - 10.0 * PI;

    if t == 0.0 {
        1.0
    } else {
        t.sin() / t
    }
}

fn random() -> f64 {
    rand::random::<f64>()
}

pub(crate) fn random_interval() -> f64 {
    (random() * 1000.0) % 600.0 + random()
}

fn default(value: f64) -> f64 {
    4.0 / PI * sin(value)
        + 4.0 / 3.0 * PI * sin(3.0 * value)
        + 4.0 / 5.0 * PI * sin(5.0 * value)
        + 4.0 / 7.0 * PI * sin(7.0 * value)
}
