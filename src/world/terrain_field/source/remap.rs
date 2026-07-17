//! Deterministic value remapping for imported field masks (ADR-102).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::super::source_error::TerrainFieldSourceError;

/// Remap decoded input samples to authoritative `u16` field values.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TerrainFieldValueRemap {
    pub input_min: u32,
    pub input_max: u32,
    pub output_min: u16,
    pub output_max: u16,
    pub invert: bool,
    pub clamp: bool,
}

impl Default for TerrainFieldValueRemap {
    fn default() -> Self {
        Self {
            input_min: 0,
            input_max: 65_535,
            output_min: 0,
            output_max: 65_535,
            invert: false,
            clamp: true,
        }
    }
}

impl TerrainFieldValueRemap {
    pub fn full_range() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), TerrainFieldSourceError> {
        if self.input_max < self.input_min {
            return Err(TerrainFieldSourceError::SourceValueRemapInvalid(
                "input_max < input_min".to_string(),
            ));
        }
        if self.input_min == self.input_max {
            return Err(TerrainFieldSourceError::SourceValueRemapInvalid(
                "input range is zero".to_string(),
            ));
        }
        Ok(())
    }

    pub fn apply(&self, input: u32) -> Result<u16, TerrainFieldSourceError> {
        self.validate()?;
        let mut value = input;
        if self.clamp {
            value = value.clamp(self.input_min, self.input_max);
        }
        let range = (self.input_max - self.input_min) as u64;
        let t = ((value - self.input_min) as u64)
            .checked_mul(65_535)
            .ok_or(TerrainFieldSourceError::GenerationOverflow)?;
        let mut out = (t / range) as u32;
        if self.invert {
            out = 65_535 - out.min(65_535);
        }
        let span = self.output_max as u32 - self.output_min as u32;
        let scaled = self.output_min as u32 + (out * span / 65_535);
        Ok(scaled.min(u16::MAX as u32) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_endpoints() {
        let remap = TerrainFieldValueRemap::full_range();
        assert_eq!(remap.apply(0).unwrap(), 0);
        assert_eq!(remap.apply(65_535).unwrap(), 65_535);
    }

    #[test]
    fn eight_bit_input_range() {
        let remap = TerrainFieldValueRemap {
            input_min: 0,
            input_max: 255,
            ..Default::default()
        };
        assert_eq!(remap.apply(255).unwrap(), 65_535);
    }
}
