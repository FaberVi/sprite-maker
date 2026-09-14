use crate::error::{CommandError, CommandResult};
use crate::models::{GenerationOptions, ProviderRequestOptions};
use std::path::Path;

pub(crate) fn provider_arguments(
    provider_id: &str,
    session_id: Option<&str>,
    model: Option<&str>,
    reasoning_effort: Option<&str>,
    reference_paths: &[String],
    prompt_file: Option<&Path>,
) -> Vec<String> {
    match provider_id {
        "codex" => codex_arguments(session_id, model, reasoning_effort, reference_paths),
        "claude" => {
            let mut arguments = vec![
                "--print".into(),
                "--input-format".into(),
                "text".into(),
                "--output-format".into(),
                "stream-json".into(),
                "--include-partial-messages".into(),
                "--verbose".into(),
                "--permission-mode".into(),
                "acceptEdits".into(),
            ];
            if let Some(session_id) = session_id {
                arguments.extend(["--resume".into(), session_id.into()]);
            }
            if let Some(model) = model {
                arguments.extend(["--model".into(), model.into()]);
            }
            if let Some(effort) = reasoning_effort {
                arguments.extend(["--effort".into(), effort.into()]);
            }
            arguments
        }
        "gemini" => {
            let mut arguments = vec![
                "--output-format".into(),
                "stream-json".into(),
                "--approval-mode".into(),
                "auto_edit".into(),
            ];
            if let Some(session_id) = session_id {
                arguments.extend(["--resume".into(), session_id.into()]);
            }
            if let Some(model) = model {
                arguments.extend(["--model".into(), model.into()]);
            }
            arguments
        }
        "grok" => {
            let mut arguments = vec![
                "--output-format".into(),
                "streaming-messages-json".into(),
                "--include-partial-messages".into(),
                "--permission-mode".into(),
                "acceptEdits".into(),
            ];
            if let Some(path) = prompt_file {
                arguments.extend(["--prompt-file".into(), path.to_string_lossy().into_owned()]);
            }
            if let Some(session_id) = session_id {
                arguments.extend(["--resume".into(), session_id.into()]);
            }
            if let Some(model) = model {
                arguments.extend(["--model".into(), model.into()]);
            }
            if let Some(effort) = reasoning_effort {
                arguments.extend(["--reasoning-effort".into(), effort.into()]);
            }
            arguments
        }
        "cursor" => {
            let mut arguments = vec![
                "-p".into(),
                "--output-format".into(),
                "stream-json".into(),
                "--stream-partial-output".into(),
                "--force".into(),
                "--trust".into(),
            ];
            if let Some(session_id) = session_id {
                arguments.extend(["--resume".into(), session_id.into()]);
            }
            if let Some(model) = model {
                arguments.extend(["--model".into(), model.into()]);
            }
            for path in reference_paths {
                arguments.extend(["--image".into(), path.clone()]);
            }
            arguments
        }
        "antigravity" => {
            let mut arguments = vec![
                "--output-format".into(),
                "stream-json".into(),
                "--input-format".into(),
                "stream-json".into(),
                "--dangerously-skip-permissions".into(),
                "--print-timeout".into(),
                "45m".into(),
            ];
            if let Some(session_id) = session_id {
                arguments.extend(["--conversation".into(), session_id.into()]);
            }
            if let Some(model) = model {
                arguments.extend(["--model".into(), model.into()]);
            }
            if let Some(effort) = reasoning_effort.filter(|value| !value.is_empty()) {
                arguments.extend(["--effort".into(), effort.into()]);
            }
            arguments
        }
        _ => Vec::new(),
    }
}

pub(crate) fn provider_stdin_bytes(provider_id: &str, prompt: &str) -> Vec<u8> {
    if provider_id == "antigravity" {
        let payload = serde_json::json!({
            "event": "user",
            "message": { "content": prompt }
        });
        let mut bytes = serde_json::to_vec(&payload).unwrap_or_else(|_| prompt.as_bytes().to_vec());
        bytes.push(b'\n');
        bytes
    } else {
        prompt.as_bytes().to_vec()
    }
}

pub(crate) fn codex_arguments(
    session_id: Option<&str>,
    model: Option<&str>,
    reasoning_effort: Option<&str>,
    reference_paths: &[String],
) -> Vec<String> {
    // Codex's Windows workspace sandbox currently has a provisioning failure
    // (`helper_unknown_error: setup refresh had errors`) that prevents every
    // tool call, including copying an ImageGen result into the workspace. The
    // other local CLI providers already run in trusted auto-edit modes, so use
    // Codex's equivalent mode on Windows until the upstream helper is reliable.
    // Keep the scoped workspace sandbox everywhere it is supported.
    let sandbox_mode = if cfg!(windows) {
        "danger-full-access"
    } else {
        "workspace-write"
    };
    let mut arguments = if session_id.is_some() {
        vec![
            "exec".into(),
            "--sandbox".into(),
            sandbox_mode.into(),
            "resume".into(),
            "--json".into(),
            "--skip-git-repo-check".into(),
        ]
    } else {
        vec![
            "exec".into(),
            "--sandbox".into(),
            sandbox_mode.into(),
            "--json".into(),
            "--skip-git-repo-check".into(),
        ]
    };
    if let Some(model) = model {
        arguments.extend(["--model".into(), model.into()]);
    }
    if let Some(reasoning_effort) = reasoning_effort {
        arguments.extend([
            "--config".into(),
            format!("model_reasoning_effort=\"{reasoning_effort}\""),
        ]);
    }
    for path in reference_paths {
        arguments.extend(["--image".into(), path.clone()]);
    }
    if let Some(session_id) = session_id {
        arguments.push(session_id.into());
    }
    arguments.push("-".into());
    arguments
}

pub(crate) fn validate_provider_options(options: &ProviderRequestOptions) -> CommandResult<()> {
    if let Some(model) = options.model.as_deref() {
        if model.is_empty()
            || model.len() > 96
            || !model
                .chars()
                .all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.'))
        {
            return Err(CommandError::new(
                "invalid_provider_model",
                "Choose a model reported by the selected provider",
            ));
        }
    }
    if let Some(effort) = options.reasoning_effort.as_deref() {
        if !matches!(
            effort,
            "minimal" | "low" | "medium" | "high" | "xhigh" | "max" | "ultra"
        ) {
            return Err(CommandError::new(
                "invalid_reasoning_effort",
                "Choose a reasoning level reported by the selected model",
            ));
        }
    }
    if let Some(command) = options.command.as_deref() {
        if !matches!(
            command,
            "animate" | "sprite" | "character" | "effect" | "pack" | "rig"
        ) {
            return Err(CommandError::new(
                "invalid_slash_command",
                "Choose a supported Sprite Studio slash command",
            ));
        }
    }
    if let Some(generation) = options.generation.as_ref() {
        validate_generation_options(generation)?;
    }
    Ok(())
}

fn validate_generation_options(generation: &GenerationOptions) -> CommandResult<()> {
    if !matches!(
        generation.quality.as_str(),
        "low" | "mid" | "high" | "custom"
    ) || !(8..=512).contains(&generation.width)
        || !(8..=512).contains(&generation.height)
        || !(1..=64).contains(&generation.frames)
        || !(1..=60).contains(&generation.fps)
        || !matches!(generation.frame_mode.as_str(), "fixed" | "auto")
        || !(1..=64).contains(&generation.min_frames)
        || !(1..=64).contains(&generation.max_frames)
        || generation.min_frames > generation.max_frames
    {
        return Err(CommandError::new(
            "invalid_generation_profile",
            "Use an 8–512 px canvas, Fixed or Auto frames within 1–64, and 1–60 FPS",
        ));
    }
    Ok(())
}
