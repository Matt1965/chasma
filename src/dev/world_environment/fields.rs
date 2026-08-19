//! Environment field definitions for World window controls (Slice 11).

use bevy::prelude::*;

use crate::environment::{
    EnvironmentManualLighting, EnvironmentSettings, TimeOfDaySettings, apply_manual_lighting,
};

/// Stable field identity for sliders, numeric entry, and drafts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnvFieldId {
    TimeHours,
    DayLengthSeconds,
    NoonDirectional,
    NightDirectional,
    NoonAmbient,
    NightAmbientMult,
    TwilightBlend,
    SunPitchMin,
    SunPitchMax,
    SunriseHour,
    SunsetHour,
    ManualDirectional,
    ManualAmbient,
}

#[derive(Debug, Clone, Copy)]
pub struct EnvFieldSpec {
    pub id: EnvFieldId,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub precision: usize,
    pub signed: bool,
    pub tooltip: &'static str,
}

impl EnvFieldId {
    pub const ALL: [EnvFieldId; 13] = [
        Self::TimeHours,
        Self::DayLengthSeconds,
        Self::NoonDirectional,
        Self::NightDirectional,
        Self::NoonAmbient,
        Self::NightAmbientMult,
        Self::TwilightBlend,
        Self::SunPitchMin,
        Self::SunPitchMax,
        Self::SunriseHour,
        Self::SunsetHour,
        Self::ManualDirectional,
        Self::ManualAmbient,
    ];

    pub fn spec(self) -> EnvFieldSpec {
        match self {
            Self::TimeHours => EnvFieldSpec {
                id: self,
                label: "Current time",
                min: 0.0,
                max: 24.0,
                precision: 2,
                signed: false,
                tooltip: "Visual clock hour (0–24). Runtime-only — not saved in Project Defaults. \
                          Changes apply immediately; does not pause the cycle.",
            },
            Self::DayLengthSeconds => EnvFieldSpec {
                id: self,
                label: "Day length",
                min: 30.0,
                max: 3600.0,
                precision: 0,
                signed: false,
                tooltip: "Real-time seconds for one full 24-hour visual cycle. Saved in Project \
                          Defaults. Lower values speed up dawn/dusk transitions.",
            },
            Self::NoonDirectional => EnvFieldSpec {
                id: self,
                label: "Noon directional",
                min: 0.0,
                max: 50_000.0,
                precision: 0,
                signed: false,
                tooltip: "Directional illuminance at solar noon (lux). Blends toward night value \
                          through twilight. Saved in Project Defaults.",
            },
            Self::NightDirectional => EnvFieldSpec {
                id: self,
                label: "Night directional",
                min: 0.0,
                max: 5_000.0,
                precision: 0,
                signed: false,
                tooltip: "Directional illuminance at deep night (lux). Saved in Project Defaults.",
            },
            Self::NoonAmbient => EnvFieldSpec {
                id: self,
                label: "Noon ambient",
                min: 0.0,
                max: 2_000.0,
                precision: 0,
                signed: false,
                tooltip: "Global ambient brightness at solar noon. Night ambient is this value \
                          multiplied by Night ambient ×. Saved in Project Defaults.",
            },
            Self::NightAmbientMult => EnvFieldSpec {
                id: self,
                label: "Night ambient ×",
                min: 0.0,
                max: 2.0,
                precision: 2,
                signed: false,
                tooltip: "Multiplier applied to noon ambient at full night. 0 yields very dark \
                          ambient; values above 1 brighten night fill. Saved in Project Defaults.",
            },
            Self::TwilightBlend => EnvFieldSpec {
                id: self,
                label: "Twilight blend",
                min: 0.0,
                max: 1.0,
                precision: 2,
                signed: false,
                tooltip: "Extra daylight factor from twilight warmth near sunrise/sunset (0–1). \
                          Higher values brighten dawn/dusk before direct sun peaks. Saved in \
                          Project Defaults.",
            },
            Self::SunPitchMin => EnvFieldSpec {
                id: self,
                label: "Sun pitch min",
                min: -90.0,
                max: 90.0,
                precision: 1,
                signed: true,
                tooltip: "Minimum sun elevation in degrees at night horizon. Saved in Project \
                          Defaults.",
            },
            Self::SunPitchMax => EnvFieldSpec {
                id: self,
                label: "Sun pitch max",
                min: -90.0,
                max: 90.0,
                precision: 1,
                signed: true,
                tooltip: "Maximum sun elevation in degrees at solar noon. Saved in Project \
                          Defaults.",
            },
            Self::SunriseHour => EnvFieldSpec {
                id: self,
                label: "Sunrise hour",
                min: 0.0,
                max: 23.0,
                precision: 1,
                signed: false,
                tooltip: "Clock hour when direct sunlight begins (0–23). Must be less than sunset. \
                          Saved in Project Defaults.",
            },
            Self::SunsetHour => EnvFieldSpec {
                id: self,
                label: "Sunset hour",
                min: 0.0,
                max: 24.0,
                precision: 1,
                signed: false,
                tooltip: "Clock hour when direct sunlight ends (0–24). Must be greater than sunrise. \
                          Saved in Project Defaults.",
            },
            Self::ManualDirectional => EnvFieldSpec {
                id: self,
                label: "Manual directional",
                min: 0.0,
                max: 50_000.0,
                precision: 0,
                signed: false,
                tooltip: "Directional illuminance when the visual cycle is disabled. Saved in \
                          Project Defaults. Ignored while cycle is enabled.",
            },
            Self::ManualAmbient => EnvFieldSpec {
                id: self,
                label: "Manual ambient",
                min: 0.0,
                max: 2_000.0,
                precision: 0,
                signed: false,
                tooltip: "Ambient brightness when the visual cycle is disabled. Saved in Project \
                          Defaults.",
            },
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }

    pub fn from_u32(id: u32) -> Option<Self> {
        match id {
            0 => Some(Self::TimeHours),
            1 => Some(Self::DayLengthSeconds),
            2 => Some(Self::NoonDirectional),
            3 => Some(Self::NightDirectional),
            4 => Some(Self::NoonAmbient),
            5 => Some(Self::NightAmbientMult),
            6 => Some(Self::TwilightBlend),
            7 => Some(Self::SunPitchMin),
            8 => Some(Self::SunPitchMax),
            9 => Some(Self::SunriseHour),
            10 => Some(Self::SunsetHour),
            11 => Some(Self::ManualDirectional),
            12 => Some(Self::ManualAmbient),
            _ => None,
        }
    }

    pub fn read(
        self,
        time_of_day: &TimeOfDaySettings,
        _environment: &EnvironmentSettings,
        manual: &EnvironmentManualLighting,
    ) -> f32 {
        match self {
            Self::TimeHours => time_of_day.time_hours,
            Self::DayLengthSeconds => time_of_day.day_length_seconds,
            Self::NoonDirectional => time_of_day.noon_directional_illuminance,
            Self::NightDirectional => time_of_day.night_directional_illuminance,
            Self::NoonAmbient => time_of_day.noon_ambient_brightness,
            Self::NightAmbientMult => time_of_day.night_ambient_multiplier,
            Self::TwilightBlend => time_of_day.twilight_daylight_blend,
            Self::SunPitchMin => time_of_day.sun_pitch_min_deg,
            Self::SunPitchMax => time_of_day.sun_pitch_max_deg,
            Self::SunriseHour => time_of_day.sunrise_hour,
            Self::SunsetHour => time_of_day.sunset_hour,
            Self::ManualDirectional => manual.values.directional_illuminance,
            Self::ManualAmbient => manual.values.ambient_brightness,
        }
    }

    pub fn write(
        self,
        value: f32,
        time_of_day: &mut TimeOfDaySettings,
        environment: &mut EnvironmentSettings,
        manual: &mut EnvironmentManualLighting,
    ) {
        match self {
            Self::TimeHours => time_of_day.set_time_hours(value),
            Self::DayLengthSeconds => time_of_day.day_length_seconds = value,
            Self::NoonDirectional => time_of_day.noon_directional_illuminance = value,
            Self::NightDirectional => time_of_day.night_directional_illuminance = value,
            Self::NoonAmbient => time_of_day.noon_ambient_brightness = value,
            Self::NightAmbientMult => time_of_day.night_ambient_multiplier = value,
            Self::TwilightBlend => time_of_day.twilight_daylight_blend = value,
            Self::SunPitchMin => time_of_day.sun_pitch_min_deg = value,
            Self::SunPitchMax => time_of_day.sun_pitch_max_deg = value,
            Self::SunriseHour => time_of_day.sunrise_hour = value,
            Self::SunsetHour => time_of_day.sunset_hour = value,
            Self::ManualDirectional => manual.values.directional_illuminance = value,
            Self::ManualAmbient => manual.values.ambient_brightness = value,
        }
        if matches!(self, Self::ManualDirectional | Self::ManualAmbient) && !time_of_day.enabled {
            apply_manual_lighting(environment, &manual.values);
        }
    }

    pub fn is_manual_only(self) -> bool {
        matches!(self, Self::ManualDirectional | Self::ManualAmbient)
    }

    pub fn is_runtime_only(self) -> bool {
        matches!(self, Self::TimeHours)
    }
}

pub fn fields_for_section(section: EnvSection) -> &'static [EnvFieldId] {
    match section {
        EnvSection::TimeCycle => &[EnvFieldId::TimeHours, EnvFieldId::DayLengthSeconds],
        EnvSection::DayLighting => &[
            EnvFieldId::NoonDirectional,
            EnvFieldId::NoonAmbient,
            EnvFieldId::SunPitchMax,
        ],
        EnvSection::NightLighting => &[
            EnvFieldId::NightDirectional,
            EnvFieldId::NightAmbientMult,
            EnvFieldId::SunPitchMin,
        ],
        EnvSection::Twilight => &[
            EnvFieldId::SunriseHour,
            EnvFieldId::SunsetHour,
            EnvFieldId::TwilightBlend,
        ],
        EnvSection::ManualLighting => &[EnvFieldId::ManualDirectional, EnvFieldId::ManualAmbient],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvSection {
    TimeCycle,
    DayLighting,
    NightLighting,
    Twilight,
    ManualLighting,
}
