// This file is part of Astarte.
//
// Copyright 2024 SECO Mind Srl
//
// SPDX-License-Identifier: Apache-2.0

//! Math functions used to simulate values sensored by the device to be sent to Astarte

use f64::consts::PI;
use std::f64;
use std::fmt::Display;
use std::time::{SystemTime, SystemTimeError};

/// Value used to derive other values to be sent to Astarte
#[derive(Clone, Copy, Debug)]
pub struct BaseValue {
    value: f64,
    scale: f64,
}

impl BaseValue {
    /// Constructor
    pub fn new(value: f64, scale: f64) -> Self {
        Self { value, scale }
    }

    /// Construct with internal value derived from the system time
    pub fn try_from_system_time(value: SystemTime, scale: f64) -> Result<Self, SystemTimeError> {
        value
            .elapsed()
            .map(|t| BaseValue::new(t.as_secs_f64(), scale))
    }

    /// Get the inner value
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Update the inner value
    pub fn update(&mut self) {
        self.value += PI * 2.0 * random_interval() * self.scale;
    }
}

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
    /// Sinc
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

impl From<String> for MathFunction {
    fn from(value: String) -> Self {
        match value.as_str() {
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
    value.sin() + rand::random::<f64>() / f64::MAX * 0.2
}

fn random_spikes_sin(value: f64) -> f64 {
    if value.sin() + rand::random::<f64>() / f64::MAX * 0.1 + rand::random::<f64>() / f64::MAX * 0.1
        > 0.999
    {
        100.0
    } else {
        0.0
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

/// Normalized sinc function: sin(PI*x) / (PI*x)
fn sinc(value: f64) -> f64 {
    if value == 0.0 {
        1.0
    } else {
        let t = value * PI;
        t.sin() / t
    }
}

fn random() -> f64 {
    rand::random::<f64>() / f64::MAX
}

fn random_interval() -> f64 {
    (rand::random::<f64>() % 600.0) * 1000.0 + (rand::random::<f64>() % 1000.0)
}

fn default(value: f64) -> f64 {
    4.0 / PI * sin(value)
        + 4.0 / 3.0 * PI * sin(3.0 * value)
        + 4.0 / 5.0 * PI * sin(5.0 * value)
        + 4.0 / 7.0 * PI * sin(7.0 * value)
}
