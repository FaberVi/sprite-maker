use super::types::{TemplateBone, TemplatePoint};

pub(super) static OBJECT_POINTS: &[TemplatePoint] = &[
    TemplatePoint {
        name: "top_left",
        kind: "anchor",
        nx: 0.2,
        ny: 0.2,
    },
    TemplatePoint {
        name: "top_right",
        kind: "anchor",
        nx: 0.8,
        ny: 0.2,
    },
    TemplatePoint {
        name: "bottom_left",
        kind: "anchor",
        nx: 0.2,
        ny: 0.8,
    },
    TemplatePoint {
        name: "bottom_right",
        kind: "anchor",
        nx: 0.8,
        ny: 0.8,
    },
    TemplatePoint {
        name: "center",
        kind: "pivot",
        nx: 0.5,
        ny: 0.5,
    },
];

pub(super) static OBJECT_BONES: &[TemplateBone] = &[TemplateBone {
    name: "body",
    start: "top_left",
    end: "bottom_right",
    radius_factor: 2.0,
    parent: None,
    z: 1,
}];

pub(super) static AMPHORPHOUS_POINTS: &[TemplatePoint] = &[
    TemplatePoint {
        name: "core",
        kind: "pivot",
        nx: 0.5,
        ny: 0.5,
    },
    TemplatePoint {
        name: "top",
        kind: "anchor",
        nx: 0.5,
        ny: 0.12,
    },
    TemplatePoint {
        name: "left",
        kind: "anchor",
        nx: 0.12,
        ny: 0.5,
    },
    TemplatePoint {
        name: "right",
        kind: "anchor",
        nx: 0.88,
        ny: 0.5,
    },
    TemplatePoint {
        name: "bottom",
        kind: "anchor",
        nx: 0.5,
        ny: 0.88,
    },
];

pub(super) static AMPHORPHOUS_BONES: &[TemplateBone] = &[TemplateBone {
    name: "blob",
    start: "top",
    end: "bottom",
    radius_factor: 2.2,
    parent: None,
    z: 1,
}];
