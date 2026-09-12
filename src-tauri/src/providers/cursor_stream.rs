use super::stream::{cursor_path_detail, cursor_trim_detail, message_text, nested_string};

pub(crate) fn parse_cursor_line(
    line: &str,
    already_has_content: bool,
) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return (None, Some(line.to_string()), None);
    };
    let event_type = value
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("activity");
    let session_id = nested_string(
        &value,
        &[
            &["session_id"],
            &["sessionId"],
            &["chat_id"],
            &["chatId"],
            &["session", "id"],
        ],
    );
    if event_type == "system" {
        let activity = (value.get("subtype").and_then(|value| value.as_str()) == Some("init"))
            .then(|| "Cursor CLI session started".to_string());
        return (None, activity, session_id);
    }
    if event_type == "assistant" {
        let has_timestamp = value.get("timestamp_ms").is_some();
        let has_model_call = value.get("model_call_id").is_some();
        let text = if has_timestamp && !has_model_call {
            message_text(&value)
        } else {
            None
        };
        return (text, None, session_id);
    }
    if event_type == "tool_call" {
        return (None, cursor_tool_call_activity(&value), session_id);
    }
    if event_type == "result" {
        let text = if already_has_content {
            None
        } else {
            nested_string(&value, &[&["result"], &["response"], &["text"]])
        };
        return (text, None, session_id);
    }
    if matches!(event_type, "error" | "failed") {
        return (
            None,
            nested_string(&value, &[&["error", "message"], &["message"]])
                .or_else(|| Some("Provider reported an error".into())),
            session_id,
        );
    }
    (None, None, session_id)
}

fn cursor_tool_call_activity(value: &serde_json::Value) -> Option<String> {
    const METADATA_KEYS: &[&str] = &[
        "completedAtMs",
        "startedAtMs",
        "timestamp_ms",
        "timestampMs",
        "durationMs",
        "callId",
        "toolCallId",
        "tool_call_id",
        "id",
    ];
    let subtype = value
        .get("subtype")
        .and_then(|value| value.as_str())
        .unwrap_or("started");
    if subtype == "completed" {
        return None;
    }
    let tool_call = value.get("tool_call")?;
    let object = tool_call.as_object()?;
    let (name, tool) = object.iter().find_map(|(key, tool)| {
        if METADATA_KEYS.contains(&key.as_str()) || tool.is_number() || tool.is_null() {
            return None;
        }
        Some((key.as_str(), tool))
    })?;
    let action = cursor_tool_display_name(name);
    cursor_tool_detail(name, tool)
        .map(|detail| format!("{action}: {detail}"))
        .or_else(|| Some(format!("{action}…")))
}

fn cursor_tool_display_name(raw: &str) -> &'static str {
    match raw {
        "globToolCall" => "Searching files",
        "writeToolCall" => "Writing file",
        "readToolCall" => "Reading file",
        "listToolCall" | "lsToolCall" => "Listing files",
        "shellToolCall" | "bashToolCall" | "runTerminalToolCall" => "Running command",
        "grepToolCall" | "searchToolCall" => "Searching code",
        "deleteToolCall" => "Deleting file",
        "GenerateImage"
        | "image_generator"
        | "generateImageToolCall"
        | "imageGeneratorToolCall" => "Generating image",
        _ => "Running tool",
    }
}

fn cursor_tool_detail(name: &str, tool: &serde_json::Value) -> Option<String> {
    let args = tool.get("args").or_else(|| tool.get("input"));
    if let Some(path) = args
        .and_then(|args| {
            args.get("path")
                .or_else(|| args.get("file"))
                .or_else(|| args.get("target"))
                .and_then(|value| value.as_str())
        })
        .map(cursor_path_detail)
    {
        return Some(path);
    }
    if name.contains("glob") || name.eq_ignore_ascii_case("globToolCall") {
        return args
            .and_then(|args| {
                args.get("pattern")
                    .or_else(|| args.get("glob"))
                    .and_then(|value| value.as_str())
            })
            .map(|pattern| format!("pattern `{pattern}`"));
    }
    if name.contains("image") || name == "GenerateImage" {
        return args
            .and_then(|args| args.get("description").and_then(|value| value.as_str()))
            .map(|description| cursor_trim_detail(description, 72));
    }
    if let Some(command) =
        args.and_then(|args| args.get("command").and_then(|value| value.as_str()))
    {
        return Some(cursor_trim_detail(command, 72));
    }
    None
}
