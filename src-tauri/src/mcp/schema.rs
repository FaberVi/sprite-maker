use crate::{
    assets::list_assets_inner,
    error::CommandResult,
    jobs::{load_job, queue_procedural_vfx_inner, queue_sprite_sheet_inner},
    models::{ProceduralVfxInput, ProviderStatus, SpriteSheetInput},
    packs::list_asset_packs_inner,
    providers::cancel_provider_request_inner,
    AppState,
};
use rmcp::{
    handler::server::wrapper::Parameters, schemars, tool, tool_handler, tool_router,
    transport::stdio, ServerHandler, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

pub(crate) const DEFAULT_PROVIDER: &str = "codex";

#[derive(Clone)]
pub struct SpriteStudioMcp {
    pub(crate) state: AppState,
    pub(crate) db_path: PathBuf,
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        tauri::async_runtime::set(tokio::runtime::Handle::current());
        let (state, db_path) = AppState::open_headless()?;
        let server = SpriteStudioMcp { state, db_path };
        let service = server.serve(stdio()).await?;
        let _ = service.waiting().await?;
        Ok(())
    })
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct OpenWorkspaceParams {
    pub(crate) path: String,
    pub(crate) name: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct EnsureConversationParams {
    #[serde(rename = "workspaceId")]
    pub(crate) workspace_id: String,
    pub(crate) provider: Option<String>,
    #[serde(rename = "stylePreset")]
    pub(crate) style_preset: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct AttachReferencesParams {
    #[serde(rename = "conversationId")]
    pub(crate) conversation_id: String,
    pub(crate) paths: Option<Vec<String>>,
    #[serde(rename = "referenceIds")]
    pub(crate) reference_ids: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct GenerateParams {
    #[serde(rename = "conversationId")]
    pub(crate) conversation_id: String,
    pub(crate) prompt: String,
    pub(crate) generation: Option<McpGenerationOptions>,
    pub(crate) command: Option<String>,
    #[serde(rename = "imageProviderId")]
    pub(crate) image_provider_id: Option<String>,
    pub(crate) model: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct McpGenerationOptions {
    pub(crate) quality: Option<String>,
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
    pub(crate) frames: Option<u32>,
    pub(crate) fps: Option<u32>,
    #[serde(rename = "frameMode")]
    pub(crate) frame_mode: Option<String>,
    #[serde(rename = "minFrames")]
    pub(crate) min_frames: Option<u32>,
    #[serde(rename = "maxFrames")]
    pub(crate) max_frames: Option<u32>,
    #[serde(rename = "allowInterpolation", default)]
    pub(crate) allow_interpolation: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct RequestIdParams {
    #[serde(rename = "requestId")]
    pub(crate) request_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct WorkspaceIdParams {
    #[serde(rename = "workspaceId")]
    pub(crate) workspace_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct ExportParams {
    pub(crate) kind: String,
    pub(crate) id: String,
    pub(crate) destination: Option<String>,
    #[serde(rename = "projectId")]
    pub(crate) project_id: Option<String>,
    #[serde(rename = "worktreeId")]
    pub(crate) worktree_id: Option<String>,
    pub(crate) name: Option<String>,
    #[serde(rename = "tileWidth")]
    pub(crate) tile_width: Option<u32>,
    #[serde(rename = "tileHeight")]
    pub(crate) tile_height: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct JobIdParams {
    #[serde(rename = "jobId")]
    pub(crate) job_id: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct QualityParams {
    #[serde(rename = "animationId")]
    pub(crate) animation_id: String,
    pub(crate) analyze: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StatusPayload {
    pub(crate) version: &'static str,
    pub(crate) db_path: String,
    pub(crate) db_ok: bool,
    pub(crate) providers: Vec<ProviderStatus>,
}

#[tool_router]
impl SpriteStudioMcp {
    #[tool(
        description = "Sprite Studio version, database path, and detected CLI/image providers. Never returns API keys."
    )]
    fn studio_status(&self) -> String {
        json_result(super::handlers::studio_status(&self.state, &self.db_path))
    }

    #[tool(description = "Open or create a Sprite Studio workspace at a local folder path.")]
    fn open_workspace(&self, Parameters(params): Parameters<OpenWorkspaceParams>) -> String {
        json_result(super::handlers::open_or_create_workspace(
            &self.state,
            params,
        ))
    }

    #[tool(
        description = "Create a chat in a workspace. Defaults to Codex so a Cursor MCP client does not nest Cursor CLI."
    )]
    fn ensure_conversation(
        &self,
        Parameters(params): Parameters<EnsureConversationParams>,
    ) -> String {
        json_result(super::handlers::ensure_conversation(&self.state, params))
    }

    #[tool(description = "Attach local image files or existing reference IDs to a conversation.")]
    fn attach_references(&self, Parameters(params): Parameters<AttachReferencesParams>) -> String {
        json_result(super::handlers::attach_references(&self.state, params))
    }

    #[tool(
        description = "Start a harness-aware sprite generation. Returns requestId; poll with get_generation."
    )]
    fn generate(&self, Parameters(params): Parameters<GenerateParams>) -> String {
        json_result(super::handlers::generate(&self.state, params))
    }

    #[tool(
        description = "Poll a generation started in this MCP process. Cancel only works for jobs started here."
    )]
    fn get_generation(&self, Parameters(params): Parameters<RequestIdParams>) -> String {
        json_result(super::handlers::get_generation(
            &self.state,
            &params.request_id,
        ))
    }

    #[tool(description = "Cancel a generation started in this MCP process.")]
    fn cancel_generation(&self, Parameters(params): Parameters<RequestIdParams>) -> String {
        json_result(
            cancel_provider_request_inner(&params.request_id, &self.state)
                .map(|_| serde_json::json!({ "cancelled": true, "requestId": params.request_id })),
        )
    }

    #[tool(
        description = "Scan the latest generation manifest and list workspace-relative assets/ files."
    )]
    fn list_artifacts(&self, Parameters(params): Parameters<WorkspaceIdParams>) -> String {
        json_result(super::handlers::list_artifacts(
            &self.state,
            &params.workspace_id,
        ))
    }

    #[tool(
        description = "Export an asset PNG, animation spritesheet, or Godot tileset. kind is asset, animation, or godot_tileset."
    )]
    fn export(&self, Parameters(params): Parameters<ExportParams>) -> String {
        json_result(super::handlers::export_item(&self.state, params))
    }

    #[tool(description = "Queue a sprite sheet composite job.")]
    fn queue_sprite_sheet(&self, Parameters(input): Parameters<McpSpriteSheetInput>) -> String {
        json_result(queue_sprite_sheet_inner(
            input.into_input(),
            None,
            &self.state,
        ))
    }

    #[tool(description = "Queue a procedural VFX job. Requires a VFX worktree.")]
    fn queue_procedural_vfx(&self, Parameters(input): Parameters<McpProceduralVfxInput>) -> String {
        json_result(queue_procedural_vfx_inner(
            input.into_input(),
            None,
            &self.state,
        ))
    }

    #[tool(description = "Load a background job (sprite sheet, VFX, or quality analysis).")]
    fn get_job(&self, Parameters(params): Parameters<JobIdParams>) -> String {
        json_result(load_job(&self.state, &params.job_id))
    }

    #[tool(
        description = "Get the latest quality report for an animation, or queue analysis when analyze is true."
    )]
    fn quality_report(&self, Parameters(params): Parameters<QualityParams>) -> String {
        json_result(super::handlers::quality_report(&self.state, params))
    }

    #[tool(description = "List indexed assets in a workspace.")]
    fn list_assets(&self, Parameters(params): Parameters<WorkspaceIdParams>) -> String {
        json_result(list_assets_inner(&params.workspace_id, &self.state))
    }

    #[tool(description = "List asset packs in a workspace.")]
    fn list_packs(&self, Parameters(params): Parameters<WorkspaceIdParams>) -> String {
        json_result(list_asset_packs_inner(&params.workspace_id, &self.state))
    }
}

#[tool_handler(
    name = "sprite-studio",
    instructions = "Headless Sprite Studio MCP. Default chat provider is Codex so a Cursor client does not nest Cursor CLI. This is not the per-workspace Python server at .sprite-studio/sprite_rig_mcp.py."
)]
impl ServerHandler for SpriteStudioMcp {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct McpSpriteSheetInput {
    #[serde(rename = "projectId")]
    pub(crate) project_id: String,
    #[serde(rename = "worktreeId")]
    pub(crate) worktree_id: Option<String>,
    #[serde(rename = "animationId")]
    pub(crate) animation_id: String,
    pub(crate) name: String,
    pub(crate) layout: String,
    #[serde(rename = "frameWidth")]
    pub(crate) frame_width: u32,
    #[serde(rename = "frameHeight")]
    pub(crate) frame_height: u32,
    pub(crate) padding: u32,
    pub(crate) spacing: u32,
    pub(crate) columns: u32,
    pub(crate) scale: u32,
    pub(crate) transparent: bool,
    pub(crate) alignment: String,
    #[serde(rename = "pivotX")]
    pub(crate) pivot_x: f64,
    #[serde(rename = "pivotY")]
    pub(crate) pivot_y: f64,
}

impl McpSpriteSheetInput {
    fn into_input(self) -> SpriteSheetInput {
        SpriteSheetInput {
            project_id: self.project_id,
            worktree_id: self.worktree_id,
            animation_id: self.animation_id,
            name: self.name,
            layout: self.layout,
            frame_width: self.frame_width,
            frame_height: self.frame_height,
            padding: self.padding,
            spacing: self.spacing,
            columns: self.columns,
            scale: self.scale,
            transparent: self.transparent,
            alignment: self.alignment,
            pivot_x: self.pivot_x,
            pivot_y: self.pivot_y,
        }
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub(crate) struct McpProceduralVfxInput {
    #[serde(rename = "projectId")]
    pub(crate) project_id: String,
    #[serde(rename = "worktreeId")]
    pub(crate) worktree_id: String,
    pub(crate) name: String,
    #[serde(rename = "effectType")]
    pub(crate) effect_type: String,
    #[serde(rename = "blendMode")]
    pub(crate) blend_mode: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) frames: u32,
    pub(crate) fps: u32,
    pub(crate) looping: bool,
    pub(crate) seed: u64,
}

impl McpProceduralVfxInput {
    fn into_input(self) -> ProceduralVfxInput {
        ProceduralVfxInput {
            project_id: self.project_id,
            worktree_id: self.worktree_id,
            name: self.name,
            effect_type: self.effect_type,
            blend_mode: self.blend_mode,
            width: self.width,
            height: self.height,
            frames: self.frames,
            fps: self.fps,
            looping: self.looping,
            seed: self.seed,
        }
    }
}

fn json_result<T: Serialize>(value: CommandResult<T>) -> String {
    match value {
        Ok(value) => serde_json::to_string_pretty(&value)
            .unwrap_or_else(|error| format!(r#"{{"error":{}}}"#, Value::String(error.to_string()))),
        Err(error) => serde_json::json!({
            "error": error.code,
            "message": error.message,
        })
        .to_string(),
    }
}

#[cfg(test)]
impl SpriteStudioMcp {
    pub(crate) fn listed_tool_names() -> Vec<String> {
        Self::tool_router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }
}
