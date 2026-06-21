//! Dev diagnostics and singleton validation for the environment layer (R9).

use bevy::prelude::*;

use super::lighting::EnvironmentDirectionalLight;
use super::settings::EnvironmentSettings;
use super::skybox::SkyboxCamera;

/// Counts of environment-owned entities that must remain singletons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EnvironmentSingletonReport {
    pub directional_lights: usize,
    pub environment_directional_lights: usize,
    pub skybox_cameras: usize,
}

impl EnvironmentSingletonReport {
    /// Whether singleton expectations are satisfied.
    pub fn is_valid(&self) -> bool {
        self.environment_directional_lights <= 1
            && self.directional_lights <= 1
            && self.skybox_cameras <= 1
            && self.environment_directional_lights == self.directional_lights
    }

    pub fn validation_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.directional_lights > 1 {
            errors.push(format!(
                "expected at most one DirectionalLight, found {}",
                self.directional_lights
            ));
        }
        if self.environment_directional_lights > 1 {
            errors.push(format!(
                "expected at most one EnvironmentDirectionalLight, found {}",
                self.environment_directional_lights
            ));
        }
        if self.skybox_cameras > 1 {
            errors.push(format!(
                "expected at most one SkyboxCamera, found {}",
                self.skybox_cameras
            ));
        }
        if self.directional_lights != self.environment_directional_lights {
            errors.push(format!(
                "DirectionalLight count ({}) does not match EnvironmentDirectionalLight count ({})",
                self.directional_lights, self.environment_directional_lights
            ));
        }
        errors
    }
}

/// Scan the ECS for environment singleton entities.
pub fn count_environment_singletons(
    directional: Query<(), With<DirectionalLight>>,
    environment_directional: Query<(), With<EnvironmentDirectionalLight>>,
    skybox_cameras: Query<(), With<SkyboxCamera>>,
) -> EnvironmentSingletonReport {
    EnvironmentSingletonReport {
        directional_lights: directional.iter().count(),
        environment_directional_lights: environment_directional.iter().count(),
        skybox_cameras: skybox_cameras.iter().count(),
    }
}

/// Validate singleton expectations; returns descriptive errors when violated.
pub fn validate_environment_singletons(report: &EnvironmentSingletonReport) -> Result<(), String> {
    let errors = report.validation_errors();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

/// Print the active environment configuration to the log (dev diagnostics).
pub fn log_environment_configuration(settings: &EnvironmentSettings) {
    bevy::log::info!(
        target: "chasma::environment",
        "{}",
        settings.format_debug_report()
    );
}

/// Print singleton validation results to the log (dev diagnostics).
pub fn log_environment_singleton_report(report: &EnvironmentSingletonReport) {
    if report.is_valid() {
        bevy::log::info!(
            target: "chasma::environment",
            "Environment singletons OK (directional={}, skybox_camera={})",
            report.directional_lights,
            report.skybox_cameras
        );
    } else {
        for error in report.validation_errors() {
            bevy::log::warn!(target: "chasma::environment", "{error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_singleton_report_accepts_one_of_each() {
        let report = EnvironmentSingletonReport {
            directional_lights: 1,
            environment_directional_lights: 1,
            skybox_cameras: 1,
        };
        assert!(report.is_valid());
        assert!(validate_environment_singletons(&report).is_ok());
    }

    #[test]
    fn duplicate_directional_light_fails_validation() {
        let report = EnvironmentSingletonReport {
            directional_lights: 2,
            environment_directional_lights: 2,
            skybox_cameras: 0,
        };
        assert!(!report.is_valid());
        assert!(validate_environment_singletons(&report).is_err());
    }

    #[test]
    fn mismatched_directional_markers_fail_validation() {
        let report = EnvironmentSingletonReport {
            directional_lights: 1,
            environment_directional_lights: 0,
            skybox_cameras: 0,
        };
        assert!(!report.is_valid());
    }

    #[test]
    fn duplicate_skybox_camera_fails_validation() {
        let report = EnvironmentSingletonReport {
            directional_lights: 1,
            environment_directional_lights: 1,
            skybox_cameras: 2,
        };
        assert!(!report.is_valid());
    }
}
