use crate::{
    error::CommandResult,
    models::{GenerationOptions, MotionPlan},
};

use super::plan::build_motion_plan;

#[tauri::command]
pub fn plan_motion(prompt: String, generation: GenerationOptions) -> CommandResult<MotionPlan> {
    build_motion_plan(prompt.trim(), &generation)
}
