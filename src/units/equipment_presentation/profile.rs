//! Per unit-render-profile socket → skeleton bone suffix maps.

use crate::world::equipment::EquipmentAttachmentSocket;

/// Bone node suffix used to resolve a semantic socket on a unit rig.
///
/// Matching is suffix-based on the full `Name` path built from the scene hierarchy
/// (see `bones::find_socket_bone_entity`).
pub fn bone_suffix_for_socket(
    unit_render_key: &str,
    socket: EquipmentAttachmentSocket,
) -> Option<&'static str> {
    match unit_render_key {
        "human_male" | "human_female" => human_bone_suffix(socket),
        "robot" => robot_bone_suffix(socket),
        _ => None,
    }
}

fn human_bone_suffix(socket: EquipmentAttachmentSocket) -> Option<&'static str> {
    match socket {
        // Verified against human_male/female GLB node names (Mixamo-style rig).
        EquipmentAttachmentSocket::RightHand => Some("hand_r"),
        EquipmentAttachmentSocket::LeftHand => Some("hand_l"),
        EquipmentAttachmentSocket::Head => Some("Head"),
        EquipmentAttachmentSocket::Back => Some("spine_02"),
        EquipmentAttachmentSocket::RightHip => Some("pelvis"),
    }
}

fn robot_bone_suffix(socket: EquipmentAttachmentSocket) -> Option<&'static str> {
    // Robot rig uses the same semantic suffix vocabulary where authored.
    human_bone_suffix(socket)
}
