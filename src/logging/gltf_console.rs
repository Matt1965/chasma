//! Console filtering for known-harmless Bevy glTF loader warnings.

use bevy::log::tracing::{Event, Level, Metadata};
use bevy::log::tracing_subscriber::layer::{Context, Filter};
use bevy::log::tracing_subscriber::Layer;
use bevy::log::{BoxedFmtLayer, LogPlugin};
use bevy::prelude::App;

const BEVY_GLTF_LOADER_TARGET: &str = "bevy_gltf::loader";
const UNKNOWN_VERTEX_ATTRIBUTE_PREFIX: &str = "Unknown vertex attribute ";

/// glTF extras Chasma does not consume; suppress only these exact names.
const SUPPRESSED_VERTEX_ATTRIBUTES: &[&str] = &[
    "COLOR_1",
    "COLOR_2",
    "TEXCOORD_2",
    "TEXCOORD_3",
    "TEXCOORD_4",
];

/// Returns the attribute name when `message` is a Bevy unknown-vertex-attribute warning.
pub fn parse_unknown_vertex_attribute_message(message: &str) -> Option<&str> {
    message
        .strip_prefix(UNKNOWN_VERTEX_ATTRIBUTE_PREFIX)
        .map(str::trim)
}

pub fn is_suppressed_unknown_vertex_attribute_name(name: &str) -> bool {
    SUPPRESSED_VERTEX_ATTRIBUTES.contains(&name)
}

fn event_message(event: &Event<'_>) -> String {
    let mut visitor = MessageVisitor::default();
    event.record(&mut visitor);
    visitor.message
}

fn should_suppress_bevy_gltf_unknown_vertex_attribute_warn(event: &Event<'_>) -> bool {
    let meta = event.metadata();
    if meta.target() != BEVY_GLTF_LOADER_TARGET || *meta.level() != Level::WARN {
        return false;
    }
    let message = event_message(event);
    parse_unknown_vertex_attribute_message(&message)
        .is_some_and(is_suppressed_unknown_vertex_attribute_name)
}

struct SuppressHarmlessGltfVertexAttributes;

impl<S> Filter<S> for SuppressHarmlessGltfVertexAttributes {
    fn enabled(&self, _meta: &Metadata<'_>, _cx: &Context<'_, S>) -> bool {
        true
    }

    fn event_enabled(&self, event: &Event<'_>, _cx: &Context<'_, S>) -> bool {
        !should_suppress_bevy_gltf_unknown_vertex_attribute_warn(event)
    }
}

#[derive(Default)]
struct MessageVisitor {
    message: String,
}

impl bevy::log::tracing::field::Visit for MessageVisitor {
    fn record_str(&mut self, field: &bevy::log::tracing::field::Field, value: &str) {
        if field.name() == "message" {
            self.message = value.to_owned();
        }
    }

    fn record_debug(&mut self, field: &bevy::log::tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" && self.message.is_empty() {
            self.message = format!("{value:?}").trim_matches('"').to_owned();
        }
    }
}

fn chasma_fmt_layer(_app: &mut App) -> Option<BoxedFmtLayer> {
    Some(Box::new(
        bevy::log::tracing_subscriber::fmt::Layer::default()
            .with_writer(std::io::stderr)
            .with_filter(SuppressHarmlessGltfVertexAttributes),
    ))
}

/// [`LogPlugin`] with Chasma console filters (harmless glTF vertex-attribute warnings).
pub fn configure_log_plugin() -> LogPlugin {
    LogPlugin {
        fmt_layer: chasma_fmt_layer,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_unknown_vertex_attribute_messages() {
        assert_eq!(
            parse_unknown_vertex_attribute_message("Unknown vertex attribute COLOR_1"),
            Some("COLOR_1")
        );
    }

    #[test]
    fn suppresses_only_known_attribute_names() {
        assert!(is_suppressed_unknown_vertex_attribute_name("COLOR_1"));
        assert!(is_suppressed_unknown_vertex_attribute_name("TEXCOORD_4"));
        assert!(!is_suppressed_unknown_vertex_attribute_name("POSITION"));
        assert!(!is_suppressed_unknown_vertex_attribute_name("COLOR_0"));
    }
}
