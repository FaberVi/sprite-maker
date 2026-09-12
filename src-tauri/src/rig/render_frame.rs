use image::RgbaImage;
use std::collections::HashMap;

use super::mesh::build_deform_mesh;
use super::skin::render_frame_with_mesh;
use super::types::{Rig, RigFrame};

pub(crate) fn render_frame(
    master: &RgbaImage,
    rig: &Rig,
    frame: &RigFrame,
    ownership: &[i16],
    positions: &HashMap<String, (f64, f64)>,
) -> RgbaImage {
    let mesh = (rig.bones.len() > 1).then(|| build_deform_mesh(master, rig, positions));
    render_frame_with_mesh(master, rig, frame, ownership, positions, mesh.as_ref())
}
