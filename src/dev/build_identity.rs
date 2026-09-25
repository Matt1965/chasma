//! Dev-only Git branch / commit identity resolved once at startup.

use std::path::Path;
use std::process::Command;

use bevy::prelude::*;
use bevy::ui::FocusPolicy;
use bevy::window::PrimaryWindow;

/// Resolved once from the repository containing Chasma (`CARGO_MANIFEST_DIR`).
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct DevBuildIdentity {
    pub branch: String,
    pub sha: String,
    pub dirty: bool,
}

/// Marker for the persistent bottom-left build label.
#[derive(Component, Debug)]
pub struct DevBuildIdentityOverlay;

impl DevBuildIdentity {
    /// Query Git once; never panics if Git or `.git` is missing.
    pub fn resolve() -> Self {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let sha = run_git(&["rev-parse", "--short", "HEAD"], repo_root);
        if sha.is_none() {
            return Self::unknown();
        }
        let sha = sha.expect("checked above");
        let branch = match run_git(&["branch", "--show-current"], repo_root) {
            Some(name) if !name.is_empty() => name,
            _ => "detached".to_string(),
        };
        let dirty = run_git(&["status", "--porcelain"], repo_root)
            .map(|status| !status.is_empty())
            .unwrap_or(false);
        Self { branch, sha, dirty }
    }

    fn unknown() -> Self {
        Self {
            branch: "unknown".to_string(),
            sha: "unknown".to_string(),
            dirty: false,
        }
    }

    /// `branch @ sha` with optional trailing `DIRTY`.
    pub fn identity_line(&self) -> String {
        if self.dirty {
            format!("{} @ {} DIRTY", self.branch, self.sha)
        } else {
            format!("{} @ {}", self.branch, self.sha)
        }
    }

    /// Window title for dev builds.
    pub fn window_title(&self) -> String {
        format!("Chasma DEV — {}", self.identity_line())
    }

    /// Bottom-left overlay label (`branch | sha` or `branch | sha | DIRTY`).
    pub fn overlay_label(&self) -> String {
        if self.dirty {
            format!("{} | {} | DIRTY", self.branch, self.sha)
        } else {
            format!("{} | {}", self.branch, self.sha)
        }
    }
}

fn run_git(args: &[&str], cwd: &Path) -> Option<String> {
    let output = Command::new("git").args(args).current_dir(cwd).output();
    match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
        _ => None,
    }
}

/// Apply the dev window title from resolved Git identity.
pub fn apply_dev_build_identity_window_title(
    identity: Res<DevBuildIdentity>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    if let Ok(mut window) = windows.single_mut() {
        window.title = identity.window_title().into();
    }
}

/// Spawn a tiny, non-interactive bottom-left build label.
pub fn setup_dev_build_identity_overlay(
    mut commands: Commands,
    identity: Res<DevBuildIdentity>,
) {
    commands.spawn((
        DevBuildIdentityOverlay,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            bottom: Val::Px(8.0),
            ..default()
        },
        Text::new(identity.overlay_label()),
        TextFont {
            font_size: 11.0,
            ..default()
        },
        TextColor(Color::srgba(0.72, 0.72, 0.72, 0.6)),
        FocusPolicy::Pass,
        ZIndex(450),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_clean_branch_identity() {
        let identity = DevBuildIdentity {
            branch: "agent-a/dialogue-foundation".to_string(),
            sha: "31ad78c2".to_string(),
            dirty: false,
        };
        assert_eq!(
            identity.identity_line(),
            "agent-a/dialogue-foundation @ 31ad78c2"
        );
        assert_eq!(
            identity.window_title(),
            "Chasma DEV — agent-a/dialogue-foundation @ 31ad78c2"
        );
        assert_eq!(
            identity.overlay_label(),
            "agent-a/dialogue-foundation | 31ad78c2"
        );
    }

    #[test]
    fn formats_dirty_checkout() {
        let identity = DevBuildIdentity {
            branch: "agent-c/fix-attack-layering".to_string(),
            sha: "abc12345".to_string(),
            dirty: true,
        };
        assert_eq!(
            identity.identity_line(),
            "agent-c/fix-attack-layering @ abc12345 DIRTY"
        );
        assert_eq!(
            identity.window_title(),
            "Chasma DEV — agent-c/fix-attack-layering @ abc12345 DIRTY"
        );
        assert_eq!(
            identity.overlay_label(),
            "agent-c/fix-attack-layering | abc12345 | DIRTY"
        );
    }

    #[test]
    fn formats_detached_head() {
        let identity = DevBuildIdentity {
            branch: "detached".to_string(),
            sha: "deadbeef".to_string(),
            dirty: false,
        };
        assert_eq!(identity.identity_line(), "detached @ deadbeef");
        assert_eq!(identity.overlay_label(), "detached | deadbeef");
    }

    #[test]
    fn formats_git_unavailable_fallback() {
        let identity = DevBuildIdentity::unknown();
        assert_eq!(identity.identity_line(), "unknown @ unknown");
        assert_eq!(identity.window_title(), "Chasma DEV — unknown @ unknown");
        assert_eq!(identity.overlay_label(), "unknown | unknown");
        assert!(!identity.dirty);
    }

    #[test]
    fn resolve_from_repository() {
        let identity = DevBuildIdentity::resolve();
        assert_ne!(identity.branch, "");
        assert_ne!(identity.sha, "");
        // When run inside the Chasma repo, Git should resolve real values.
        if identity.branch != "unknown" {
            assert!(
                identity.sha.len() >= 7,
                "expected short SHA, got {}",
                identity.sha
            );
        }
    }
}
