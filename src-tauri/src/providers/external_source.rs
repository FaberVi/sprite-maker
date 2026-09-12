use super::image_providers::StoredImageProvider;
use crate::error::{CommandError, CommandResult};
use base64::Engine;
use reqwest::Url;
use serde::Deserialize;
use std::{
    fs,
    net::{IpAddr, Ipv6Addr},
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct ImageApiResponse {
    data: Vec<ImageApiItem>,
}

#[derive(Debug, Deserialize)]
struct ImageApiItem {
    #[serde(default)]
    b64_json: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

pub(crate) async fn generate_external_source(
    workspace: &Path,
    provider: &StoredImageProvider,
    prompt: &str,
) -> CommandResult<PathBuf> {
    let endpoint = if provider.base_url.ends_with("/images/generations") {
        provider.base_url.clone()
    } else {
        format!(
            "{}/images/generations",
            provider.base_url.trim_end_matches('/')
        )
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| CommandError::new("provider_error", error.to_string()))?;
    let response = client.post(&endpoint)
        .bearer_auth(&provider.api_key)
        .json(&serde_json::json!({"model": provider.model, "prompt": prompt, "n": 1, "response_format": "b64_json"}))
        .send().await.map_err(|error| CommandError::new("provider_error", format!("{} request failed: {error}", provider.name)))?;
    let status = response.status();
    if !status.is_success() {
        return Err(CommandError::new(
            "provider_error",
            format!(
                "{} returned {status}. Check the endpoint, model ID, API key, and account access.",
                provider.name
            ),
        ));
    }
    let result: ImageApiResponse = response.json().await.map_err(|error| {
        CommandError::new(
            "provider_error",
            format!(
                "{} returned an invalid image response: {error}",
                provider.name
            ),
        )
    })?;
    let item = result.data.into_iter().next().ok_or_else(|| {
        CommandError::new(
            "provider_error",
            format!("{} returned no image", provider.name),
        )
    })?;
    let bytes = if let Some(encoded) = item.b64_json {
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|error| {
                CommandError::new(
                    "provider_error",
                    format!("Could not decode {} image: {error}", provider.name),
                )
            })?
    } else if let Some(url) = item.url {
        let download_url = validate_provider_image_url(&url, &endpoint)?;
        client
            .get(download_url)
            .send()
            .await
            .map_err(|error| {
                CommandError::new(
                    "provider_error",
                    format!("Could not download {} image: {error}", provider.name),
                )
            })?
            .error_for_status()
            .map_err(|error| CommandError::new("provider_error", error.to_string()))?
            .bytes()
            .await
            .map_err(|error| CommandError::new("provider_error", error.to_string()))?
            .to_vec()
    } else {
        return Err(CommandError::new(
            "provider_error",
            format!(
                "{} response contained neither image data nor a URL",
                provider.name
            ),
        ));
    };
    let image = image::load_from_memory(&bytes)?;
    let directory = workspace.join(".sprite-studio/provider-sources");
    fs::create_dir_all(&directory)?;
    let output = directory.join(format!("{}-{}.png", provider.id, Uuid::new_v4()));
    image.save_with_format(&output, image::ImageFormat::Png)?;
    Ok(output)
}

fn validate_provider_image_url(url: &str, endpoint: &str) -> CommandResult<Url> {
    let parsed = Url::parse(url).map_err(|error| {
        CommandError::new(
            "provider_error",
            format!("Provider returned an invalid image URL: {error}"),
        )
    })?;
    let scheme = parsed.scheme();
    if scheme != "https" && scheme != "http" {
        return Err(CommandError::new(
            "provider_error",
            "Provider image URLs must use HTTP or HTTPS",
        ));
    }
    let host = parsed.host_str().ok_or_else(|| {
        CommandError::new("provider_error", "Provider image URL is missing a host")
    })?;
    if blocked_download_host(host) {
        return Err(CommandError::new(
            "provider_error",
            "Provider image URL points to a blocked local or private host",
        ));
    }
    let endpoint_host = Url::parse(endpoint)
        .ok()
        .and_then(|value| value.host_str().map(str::to_ascii_lowercase));
    if endpoint_host.is_some_and(|allowed| host.eq_ignore_ascii_case(&allowed)) {
        return Ok(parsed);
    }
    if host.eq_ignore_ascii_case("oaidalleapiprodscus.blob.core.windows.net")
        || host.ends_with(".blob.core.windows.net")
        || host.ends_with(".googleusercontent.com")
        || host.ends_with(".openai.com")
    {
        return Ok(parsed);
    }
    Err(CommandError::new(
        "provider_error",
        "Provider image URL host is not allowed for download",
    ))
}

fn blocked_download_host(host: &str) -> bool {
    let normalized = host.trim_matches(['[', ']']).to_ascii_lowercase();
    if normalized == "localhost"
        || normalized.ends_with(".localhost")
        || normalized == "0.0.0.0"
    {
        return true;
    }
    if let Ok(ip) = IpAddr::from_str(&normalized) {
        return private_or_loopback_ip(ip);
    }
    false
}

fn private_or_loopback_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(value) => {
            value.is_private()
                || value.is_loopback()
                || value.is_link_local()
                || value.is_broadcast()
                || value.is_unspecified()
        }
        IpAddr::V6(value) => {
            value.is_loopback()
                || value.is_unspecified()
                || (value.segments()[0] & 0xfe00) == 0xfc00
                || value == Ipv6Addr::LOCALHOST
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{blocked_download_host, private_or_loopback_ip, validate_provider_image_url};
    use std::net::{IpAddr, Ipv4Addr};

    #[test]
    fn blocks_local_image_hosts() {
        assert!(blocked_download_host("localhost"));
        assert!(blocked_download_host("127.0.0.1"));
        assert!(private_or_loopback_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
    }

    #[test]
    fn allows_provider_blob_hosts() {
        let endpoint = "https://api.openai.com/v1/images/generations";
        let url = "https://oaidalleapiprodscus.blob.core.windows.net/private/image.png";
        assert!(validate_provider_image_url(url, endpoint).is_ok());
    }
}
