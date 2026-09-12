use super::stream::{cursor_path_detail, cursor_trim_detail, nested_string};

pub(crate) fn parse_antigravity_line(
    line: &str,
    already_has_content: bool,
) -> (Option<String>, Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return (None, Some(line.to_string()), None);
    };
    let event = value
        .get("event")
        .and_then(|value| value.as_str())
        .unwrap_or("activity");
    let session_id = nested_string(
        &value,
        &[
            &["conversation_id"],
            &["step_update", "conversation_id"],
            &["result", "conversation_id"],
        ],
    );
    match event {
        "init" => (
            None,
            Some("Antigravity CLI session started".into()),
            session_id,
        ),
        "step_update" => {
            let step = value.get("step_update").unwrap_or(&value);
            let session_id = nested_string(step, &[&["conversation_id"]]).or(session_id);
            let step_type = step
                .get("step_type")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let state = step
                .get("state")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if step_type == "tool" && state != "DONE" {
                return (None, antigravity_tool_activity(step), session_id);
            }
            if step_type == "agent_response" && state != "DONE" {
                let text = step
                    .get("text_delta")
                    .and_then(|value| value.as_str())
                    .filter(|text| !text.is_empty())
                    .map(str::to_string);
                return (text, None, session_id);
            }
            (None, None, session_id)
        }
        "result" => {
            let result = value.get("result").unwrap_or(&value);
            let session_id = nested_string(result, &[&["conversation_id"]]).or(session_id);
            let status = result
                .get("status")
                .and_then(|value| value.as_str())
                .unwrap_or("SUCCESS");
            if status != "SUCCESS" {
                let detail = nested_string(result, &[&["error"], &["response"]])
                    .unwrap_or_else(|| format!("Antigravity CLI reported {status}"));
                return (
                    Some(format!("GENERATION_FAILED: {detail}")),
                    None,
                    session_id,
                );
            }
            let text = if already_has_content {
                None
            } else {
                nested_string(result, &[&["response"], &["text"]])
            };
            (text, None, session_id)
        }
        _ => (None, None, session_id),
    }
}

fn antigravity_tool_activity(step: &serde_json::Value) -> Option<String> {
    let name = step
        .get("tool_name")
        .or_else(|| step.get("tool_info").and_then(|tool| tool.get("name")))
        .and_then(|value| value.as_str())
        .unwrap_or("tool");
    let action = antigravity_tool_display_name(name);
    let params = step
        .get("tool_info")
        .and_then(|tool| tool.get("parameters"));
    antigravity_tool_detail(name, params)
        .map(|detail| format!("{action}: {detail}"))
        .or_else(|| Some(format!("{action}…")))
}

fn antigravity_tool_display_name(raw: &str) -> &'static str {
    match raw {
        "generate_image" | "GenerateImage" => "Generating image",
        "write_to_file" | "write" | "create_file" => "Writing file",
        "view" | "read_file" | "read" => "Reading file",
        "run_command" => "Running command",
        "glob" | "search" => "Searching files",
        _ => "Running tool",
    }
}

fn antigravity_tool_detail(name: &str, params: Option<&serde_json::Value>) -> Option<String> {
    let params = params?;
    if let Some(path) = params
        .get("path")
        .or_else(|| params.get("TargetFile"))
        .or_else(|| params.get("target"))
        .or_else(|| params.get("file"))
        .and_then(|value| value.as_str())
    {
        return Some(cursor_path_detail(path));
    }
    if name == "generate_image" || name == "GenerateImage" {
        return params
            .get("prompt")
            .or_else(|| params.get("description"))
            .and_then(|value| value.as_str())
            .map(|description| cursor_trim_detail(description, 72));
    }
    if let Some(command) = params
        .get("CommandLine")
        .or_else(|| params.get("command"))
        .and_then(|value| value.as_str())
    {
        return Some(cursor_trim_detail(command, 72));
    }
    None
}
