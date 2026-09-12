use super::discovery::{
    isolate_login_shell, login_shell_state, provider_process_path, stop_login_shell,
    LoginShellState,
};
use super::headless::apply_std_headless_flags;
use crate::models::ProviderMode;
use serde::Deserialize;
use std::{
    env,
    fs::{self, OpenOptions},
    path::Path,
    process::{Command as StdCommand, Stdio},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

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

pub(crate) fn command_output(
    provider_id: &str,
    executable: &Path,
    arguments: &[&str],
) -> Option<std::process::Output> {
    const PROBE_TIMEOUT: Duration = Duration::from_secs(4);
    let token = Uuid::new_v4();
    let stdout_path = env::temp_dir().join(format!("sprite-studio-provider-{token}.out"));
    let stderr_path = env::temp_dir().join(format!("sprite-studio-provider-{token}.err"));
    let result = (|| {
        let stdout = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&stdout_path)
            .ok()?;
        let stderr = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&stderr_path)
            .ok()?;
        let mut command = StdCommand::new(executable);
        command
            .args(arguments)
            .env("PATH", provider_process_path(provider_id))
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        // Provider CLIs can leave helper processes alive after the command that
        // launched them exits. Keep every read-only probe in its own group so a
        // timeout cleans up the whole probe without touching a real generation.
        isolate_login_shell(&mut command);
        apply_std_headless_flags(&mut command);
        let mut child = command.spawn().ok()?;
        let deadline = Instant::now() + PROBE_TIMEOUT;
        let status = loop {
            match login_shell_state(&mut child) {
                LoginShellState::Exited => break stop_login_shell(&mut child, true)?,
                LoginShellState::Running => {}
                LoginShellState::Error => {
                    let _ = stop_login_shell(&mut child, false);
                    return None;
                }
            }
            if Instant::now() >= deadline {
                let _ = stop_login_shell(&mut child, false);
                return None;
            }
            thread::sleep(Duration::from_millis(10));
        };
        Some(std::process::Output {
            status,
            stdout: fs::read(&stdout_path).unwrap_or_default(),
            stderr: fs::read(&stderr_path).unwrap_or_default(),
        })
    })();
    let _ = fs::remove_file(stdout_path);
    let _ = fs::remove_file(stderr_path);
    result
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

pub(crate) fn provider_is_authenticated(id: &str, executable: &Path) -> bool {
    match id {
        "codex" => command_output(id, executable, &["login", "status"])
            .is_some_and(|output| output.status.success()),
        "claude" => command_output(id, executable, &["auth", "status", "--json"])
            .filter(|output| output.status.success())
            .and_then(|output| serde_json::from_slice::<serde_json::Value>(&output.stdout).ok())
            .and_then(|value| value.get("loggedIn").and_then(|value| value.as_bool()))
            .unwrap_or(false),
        "cursor" => {
            if cursor_api_key_configured() {
                return true;
            }
            match command_output(id, executable, &["status", "--format", "json"]) {
                Some(output) => cursor_is_authenticated(&output),
                None => command_output(id, executable, &["status"])
                    .is_some_and(|output| cursor_is_authenticated(&output)),
            }
        }
        // `models` is a read-only command which requires an authenticated Grok
        // session. It provides a stronger signal than checking credential files.
        "grok" => command_output(id, executable, &["models"])
            .is_some_and(|output| output.status.success()),
        "antigravity" => command_output(id, executable, &["models"])
            .is_some_and(|output| output.status.success()),
        // Gemini has no stable auth-status command. Treat configured credentials
        // as authenticated and defer the final check to the first headless run.
        "gemini" => gemini_credentials_available(),
        _ => false,
    }
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
