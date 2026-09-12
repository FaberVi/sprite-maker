use crate::{
    error::{CommandError, CommandResult},
    models::{
        ImageProviderInput, ProviderCapabilities, ProviderConnectionTest, ProviderMode,
        ProviderStatus,
    },
    AppState,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StoredImageProvider {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) provider_type: String,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) model: String,
}

pub(crate) fn image_provider_status(provider: &StoredImageProvider) -> ProviderStatus {
    let configured =
        !provider.api_key.is_empty() && !provider.base_url.is_empty() && !provider.model.is_empty();
    ProviderStatus {
        id: provider.id.clone(),
        name: provider.name.clone(),
        kind: "image".into(),
        installed: configured,
        executable: None,
        status: if configured { "ready" } else { "unavailable" }.into(),
        detail: if configured {
            format!("{} image API configured", provider.provider_type)
        } else {
            "Add an API key to enable this image source".into()
        },
        modes: vec![ProviderMode {
            id: provider.model.clone(),
            label: provider.model.clone(),
            description: format!("Image model served by {}", provider.name),
            default_reasoning_effort: String::new(),
            reasoning_efforts: Vec::new(),
        }],
        capabilities: ProviderCapabilities {
            text_input: true,
            image_input: false,
            multiple_image_input: false,
            image_editing: false,
            masks: false,
            transparency: false,
            structured_output: false,
            video_animation: false,
            image_to_image: false,
            maximum_reference_images: 0,
        },
        configurable: true,
        has_api_key: !provider.api_key.is_empty(),
        base_url: Some(provider.base_url.clone()),
        model: Some(provider.model.clone()),
    }
}

pub(crate) fn stored_image_providers(state: &AppState) -> CommandResult<Vec<StoredImageProvider>> {
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let mut statement = connection.prepare("SELECT settings_json FROM provider_settings WHERE provider LIKE 'image:%' AND enabled=1 ORDER BY provider")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows
        .filter_map(Result::ok)
        .filter_map(|json| serde_json::from_str(&json).ok())
        .collect())
}

pub(crate) fn load_image_provider(
    state: &AppState,
    id: &str,
) -> CommandResult<Option<StoredImageProvider>> {
    if id == "imagegen" || id == "cursor-image" || id == "antigravity-image" {
        return Ok(None);
    }
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    let json: Option<String> = connection
        .query_row(
            "SELECT settings_json FROM provider_settings WHERE provider=?1 AND enabled=1",
            [format!("image:{id}")],
            |row| row.get(0),
        )
        .optional()?;
    json.map(|value| {
        serde_json::from_str(&value)
            .map_err(|error| CommandError::new("invalid_provider", error.to_string()))
    })
    .transpose()
}

pub(crate) fn is_provider_native_image(image_provider_id: &str, provider_id: &str) -> bool {
    image_provider_id == "provider-native"
        || (image_provider_id == "imagegen" && provider_id != "codex")
        || (image_provider_id == "cursor-image" && provider_id != "cursor")
        || (image_provider_id == "antigravity-image" && provider_id != "antigravity")
}

fn validate_provider_base_url(value: &str) -> CommandResult<String> {
    let value = value.trim().trim_end_matches('/');
    let parsed = reqwest::Url::parse(value).map_err(|_| {
        CommandError::new(
            "invalid_provider",
            "Enter a valid HTTPS API base URL, such as https://api.example.com/v1",
        )
    })?;
    let local_debug_url = cfg!(debug_assertions)
        && parsed.scheme() == "http"
        && matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if (parsed.scheme() != "https" && !local_debug_url)
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(CommandError::new(
            "invalid_provider",
            "Use an HTTPS API base URL without credentials, query parameters, or fragments",
        ));
    }
    Ok(value.to_string())
}

fn provider_from_input(
    state: &AppState,
    input: ImageProviderInput,
) -> CommandResult<StoredImageProvider> {
    let id = input.id.trim().to_lowercase();
    let provider_type = input.provider_type.trim().to_lowercase();
    if id.is_empty()
        || id == "imagegen"
        || id == "cursor-image"
        || id == "antigravity-image"
        || id == "midjourney"
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(CommandError::new(
            "invalid_provider",
            "Use a simple provider ID containing letters, numbers, dashes, or underscores; reserved built-in IDs cannot be reused",
        ));
    }
    if !matches!(provider_type.as_str(), "grok" | "openai-compatible") {
        return Err(CommandError::new(
            "invalid_provider",
            "Choose Grok or an OpenAI-compatible image API",
        ));
    }
    let name = input.name.trim();
    let model = input.model.trim();
    if name.is_empty() || model.is_empty() || name.len() > 80 || model.len() > 160 {
        return Err(CommandError::new(
            "invalid_provider",
            "Display name and model ID are required",
        ));
    }
    let base_url = validate_provider_base_url(&input.base_url)?;
    let existing_key = load_image_provider(state, &id)?
        .map(|value| value.api_key)
        .unwrap_or_default();
    let api_key = if input.api_key.trim().is_empty() {
        existing_key
    } else {
        input.api_key.trim().to_string()
    };
    if api_key.is_empty() {
        return Err(CommandError::new(
            "invalid_provider",
            "An API key is required. It is stored locally and never shown after saving.",
        ));
    }
    Ok(StoredImageProvider {
        id,
        name: name.into(),
        provider_type,
        base_url,
        api_key,
        model: model.into(),
    })
}

#[tauri::command]
pub fn save_image_provider(
    input: ImageProviderInput,
    state: State<'_, AppState>,
) -> CommandResult<ProviderStatus> {
    let provider = provider_from_input(&state, input)?;
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    connection.execute(
        "INSERT INTO provider_settings(provider,enabled,settings_json,updated_at) VALUES (?1,1,?2,?3) ON CONFLICT(provider) DO UPDATE SET enabled=1,settings_json=excluded.settings_json,updated_at=excluded.updated_at",
        params![format!("image:{}", provider.id), serde_json::to_string(&provider).map_err(|error| CommandError::new("invalid_provider", error.to_string()))?, chrono::Utc::now().to_rfc3339()]
    )?;
    Ok(image_provider_status(&provider))
}

#[tauri::command]
pub async fn test_image_provider(
    input: ImageProviderInput,
    state: State<'_, AppState>,
) -> CommandResult<ProviderConnectionTest> {
    let provider = provider_from_input(&state, input)?;
    let endpoint = if provider.base_url.ends_with("/models") {
        provider.base_url.clone()
    } else {
        format!("{}/models", provider.base_url)
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| CommandError::new("provider_error", error.to_string()))?;
    let response = client
        .get(endpoint)
        .bearer_auth(&provider.api_key)
        .send()
        .await
        .map_err(|error| {
            CommandError::new(
                "provider_error",
                format!("Could not reach {}: {error}", provider.name),
            )
        })?;
    let status = response.status();
    if status.is_success() {
        return Ok(ProviderConnectionTest {
            ok: true,
            detail: format!(
                "Connected to {} and authenticated successfully",
                provider.name
            ),
        });
    }
    let detail = match status.as_u16() {
        401 | 403 => "The endpoint was reached, but it rejected the API key",
        404 => "The endpoint does not expose the OpenAI-compatible /models route",
        429 => "The endpoint was reached, but it is currently rate-limiting requests",
        _ => "The endpoint was reached, but it returned an unexpected response",
    };
    Err(CommandError::new(
        "provider_test_failed",
        format!("{detail} ({status})"),
    ))
}

#[tauri::command]
pub fn delete_image_provider(id: String, state: State<'_, AppState>) -> CommandResult<()> {
    if id == "grok-image" {
        return Err(CommandError::new(
            "provider_builtin",
            "The Grok entry can be cleared but not removed",
        ));
    }
    let connection = state
        .db
        .lock()
        .map_err(|_| CommandError::new("database_locked", "Database lock was poisoned"))?;
    connection.execute(
        "DELETE FROM provider_settings WHERE provider=?1",
        [format!("image:{id}")],
    )?;
    Ok(())
}
