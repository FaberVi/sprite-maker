use super::discovery::provider_process_path;
use super::headless::apply_std_headless_flags;
use super::probes::{
    command_output, probe_failure_detail, AuthCheck, ProbeOutcome,
};
use crate::models::ProviderMode;
use serde::Deserialize;
use std::{env, fs, path::Path, process::Command as StdCommand};

#[derive(Debug, Deserialize)]
struct CodexModelCatalog {
    models: Vec<CodexModelEntry>,
}

#[derive(Debug, Deserialize)]
struct CodexModelEntry {
    slug: String,
    display_name: String,
    description: String,
    default_reasoning_level: String,
    supported_reasoning_levels: Vec<CodexReasoningLevel>,
    visibility: String,
    priority: i64,
}

#[derive(Debug, Deserialize)]
struct CodexReasoningLevel {
    effort: String,
}

pub(crate) fn codex_modes(executable: &Path) -> Vec<ProviderMode> {
    let mut command = StdCommand::new(executable);
    command
        .args(["debug", "models"])
        .env("PATH", provider_process_path("codex"));
    apply_std_headless_flags(&mut command);
    let Ok(output) = command.output() else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let Ok(mut catalog) = serde_json::from_slice::<CodexModelCatalog>(&output.stdout) else {
        return Vec::new();
    };
    catalog.models.sort_by_key(|model| model.priority);
    catalog
        .models
        .into_iter()
        .filter(|model| model.visibility == "list")
        .map(|model| ProviderMode {
            id: model.slug,
            label: model.display_name,
            description: model.description,
            default_reasoning_effort: model.default_reasoning_level,
            reasoning_efforts: model
                .supported_reasoning_levels
                .into_iter()
                .map(|level| level.effort)
                .collect(),
        })
        .collect()
}

fn cursor_api_key_configured() -> bool {
    env::var("CURSOR_API_KEY")
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn gemini_credentials_available() -> bool {
    const API_KEYS: &[&str] = &[
        "GEMINI_API_KEY",
        "GOOGLE_API_KEY",
        "GOOGLE_GENERATIVE_AI_API_KEY",
    ];
    if API_KEYS
        .iter()
        .any(|key| env::var(key).is_ok_and(|value| !value.trim().is_empty()))
    {
        return true;
    }
    let config_dir = env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(|home| Path::new(&home).join(".gemini"));
    config_dir.is_some_and(|dir| {
        fs::read_dir(dir).is_ok_and(|entries| {
            entries.filter_map(Result::ok).any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
            })
        })
    })
}

pub(crate) fn cursor_auth_from_status_json(value: &serde_json::Value) -> Option<bool> {
    if let Some(authenticated) = value
        .get("isAuthenticated")
        .and_then(|value| value.as_bool())
    {
        return Some(authenticated);
    }
    if let Some(logged_in) = value.get("loggedIn").and_then(|value| value.as_bool()) {
        return Some(logged_in);
    }
    if let Some(authenticated) = value.get("authenticated").and_then(|value| value.as_bool()) {
        return Some(authenticated);
    }
    if let Some(has_access_token) = value
        .get("hasAccessToken")
        .and_then(|value| value.as_bool())
    {
        return Some(has_access_token);
    }
    match value.get("status").and_then(|value| value.as_str()) {
        Some(status)
            if status.eq_ignore_ascii_case("authenticated")
                || status.eq_ignore_ascii_case("logged_in") =>
        {
            Some(true)
        }
        Some(status)
            if status.eq_ignore_ascii_case("unauthenticated")
                || status.eq_ignore_ascii_case("logged_out")
                || status.eq_ignore_ascii_case("not_logged_in") =>
        {
            Some(false)
        }
        _ => None,
    }
}

fn cursor_is_authenticated(output: &std::process::Output) -> bool {
    if cursor_api_key_configured() {
        return true;
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
        return cursor_auth_from_status_json(&value).unwrap_or(false);
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if combined.contains("Not logged in") {
        return false;
    }
    false
}

pub(crate) fn check_provider_auth(id: &str, executable: &Path) -> AuthCheck {
    match id {
        "codex" => match command_output(id, executable, &["login", "status"]) {
            ProbeOutcome::Ran(output) if output.status.success() => AuthCheck::Authenticated,
            ProbeOutcome::Ran(_) => AuthCheck::NotLoggedIn,
            ProbeOutcome::Failed(failure) => AuthCheck::ProbeFailed {
                detail: probe_failure_detail(id, executable, &["login", "status"], &failure),
            },
        },
        "claude" => match command_output(id, executable, &["auth", "status", "--json"]) {
            ProbeOutcome::Ran(output) if output.status.success() => {
                let logged_in = serde_json::from_slice::<serde_json::Value>(&output.stdout)
                    .ok()
                    .and_then(|value| value.get("loggedIn").and_then(|value| value.as_bool()))
                    .unwrap_or(false);
                if logged_in {
                    AuthCheck::Authenticated
                } else {
                    AuthCheck::NotLoggedIn
                }
            }
            ProbeOutcome::Ran(_) => AuthCheck::NotLoggedIn,
            ProbeOutcome::Failed(failure) => AuthCheck::ProbeFailed {
                detail: probe_failure_detail(
                    id,
                    executable,
                    &["auth", "status", "--json"],
                    &failure,
                ),
            },
        },
        "cursor" => {
            if cursor_api_key_configured() {
                return AuthCheck::Authenticated;
            }
            match command_output(id, executable, &["status", "--format", "json"]) {
                ProbeOutcome::Ran(output) if cursor_is_authenticated(&output) => {
                    AuthCheck::Authenticated
                }
                ProbeOutcome::Ran(_) => match command_output(id, executable, &["status"]) {
                    ProbeOutcome::Ran(output) if cursor_is_authenticated(&output) => {
                        AuthCheck::Authenticated
                    }
                    ProbeOutcome::Ran(_) => AuthCheck::NotLoggedIn,
                    ProbeOutcome::Failed(failure) => AuthCheck::ProbeFailed {
                        detail: probe_failure_detail(id, executable, &["status"], &failure),
                    },
                },
                ProbeOutcome::Failed(failure) => AuthCheck::ProbeFailed {
                    detail: probe_failure_detail(
                        id,
                        executable,
                        &["status", "--format", "json"],
                        &failure,
                    ),
                },
            }
        }
        "grok" | "antigravity" => match command_output(id, executable, &["models"]) {
            ProbeOutcome::Ran(output) if output.status.success() => AuthCheck::Authenticated,
            ProbeOutcome::Ran(_) => AuthCheck::NotLoggedIn,
            ProbeOutcome::Failed(failure) => AuthCheck::ProbeFailed {
                detail: probe_failure_detail(id, executable, &["models"], &failure),
            },
        },
        "gemini" => {
            if gemini_credentials_available() {
                AuthCheck::Authenticated
            } else {
                AuthCheck::NotLoggedIn
            }
        }
        _ => AuthCheck::NotLoggedIn,
    }
}

pub(crate) fn provider_is_authenticated(id: &str, executable: &Path) -> bool {
    matches!(check_provider_auth(id, executable), AuthCheck::Authenticated)
}

pub(crate) fn cursor_modes_from_output(output: &std::process::Output) -> Vec<ProviderMode> {
    if !output.status.success() {
        return Vec::new();
    }
    if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
        let models = value
            .as_array()
            .or_else(|| value.get("models").and_then(|value| value.as_array()));
        if let Some(models) = models {
            return models
                .iter()
                .filter_map(|item| {
                    if let Some(id) = item.as_str() {
                        return (!id.is_empty()).then(|| ProviderMode {
                            id: id.to_string(),
                            label: id.to_string(),
                            description: "Model reported by the installed Cursor CLI".into(),
                            default_reasoning_effort: String::new(),
                            reasoning_efforts: Vec::new(),
                        });
                    }
                    let id = item
                        .get("id")
                        .or_else(|| item.get("slug"))
                        .or_else(|| item.get("name"))
                        .and_then(|value| value.as_str())?;
                    if id.is_empty() {
                        return None;
                    }
                    let label = item
                        .get("displayName")
                        .or_else(|| item.get("display_name"))
                        .or_else(|| item.get("label"))
                        .and_then(|value| value.as_str())
                        .unwrap_or(id);
                    Some(ProviderMode {
                        id: id.to_string(),
                        label: label.to_string(),
                        description: "Model reported by the installed Cursor CLI".into(),
                        default_reasoning_effort: String::new(),
                        reasoning_efforts: Vec::new(),
                    })
                })
                .collect();
        }
    }
    grok_modes_from_output(output)
}

pub(crate) fn antigravity_modes_from_output(output: &std::process::Output) -> Vec<ProviderMode> {
    if !output.status.success() {
        return Vec::new();
    }
    let from_json = cursor_modes_from_output(output);
    if !from_json.is_empty() {
        return from_json
            .into_iter()
            .map(|mut mode| {
                mode.description = "Model reported by the installed Antigravity CLI".into();
                if mode.reasoning_efforts.is_empty() {
                    mode.reasoning_efforts = vec!["low".into(), "medium".into(), "high".into()];
                }
                mode
            })
            .collect();
    }
    antigravity_modes_from_text(&String::from_utf8_lossy(&output.stdout))
}

pub(crate) fn antigravity_modes_from_text(stdout: &str) -> Vec<ProviderMode> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("Available"))
        .filter_map(|line| {
            let (id, label) = match line.split_once(char::is_whitespace) {
                Some((id, rest)) if !id.is_empty() => {
                    let label = rest.trim();
                    (id, if label.is_empty() { id } else { label })
                }
                _ => (line, line),
            };
            (!id.is_empty()).then(|| ProviderMode {
                id: id.to_string(),
                label: label.to_string(),
                description: "Model reported by the installed Antigravity CLI".into(),
                default_reasoning_effort: String::new(),
                reasoning_efforts: vec!["low".into(), "medium".into(), "high".into()],
            })
        })
        .collect()
}

pub(crate) fn grok_modes_from_output(output: &std::process::Output) -> Vec<ProviderMode> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            let value = line
                .strip_prefix('*')
                .or_else(|| line.strip_prefix('-'))?
                .trim()
                .strip_suffix("(default)")
                .unwrap_or_else(|| line.trim_start_matches(['*', '-']).trim())
                .trim();
            (!value.is_empty()).then(|| ProviderMode {
                id: value.to_string(),
                label: value.to_string(),
                description: "Model reported by the installed Grok CLI".into(),
                default_reasoning_effort: "medium".into(),
                reasoning_efforts: vec!["low".into(), "medium".into(), "high".into()],
            })
        })
        .collect()
}
