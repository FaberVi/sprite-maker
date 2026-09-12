use super::{
    antigravity_cli_install_command, antigravity_modes_from_text, apply_std_headless_flags,
    cursor_auth_from_status_json, cursor_cli_install_command, executable_lookup_names,
    is_provider_native_image, merge_provider_paths, provider_process_path,
};
#[cfg(unix)]
use super::{login_shell_path, login_shell_path_with_timeout, provider_environment_path};
use std::{env, path::PathBuf};

#[cfg(unix)]
#[test]
fn reads_provider_path_from_login_shell_output() {
    use std::{fs, os::unix::fs::PermissionsExt};
    use uuid::Uuid;

    let root = env::temp_dir().join(format!("sprite-studio-shell-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("test directory should exist");
    let shell = root.join("shell");
    fs::write(
            &shell,
            "#!/bin/sh\nprintf 'shell startup noise\\n__SPRITE_STUDIO_PATH__=/runtime/bin:/system/bin\\n'\n",
        )
        .expect("fake shell should be written");
    let mut permissions = fs::metadata(&shell)
        .expect("fake shell metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&shell, permissions).expect("fake shell should be executable");

    let discovered = login_shell_path(&shell);

    assert_eq!(
        discovered.as_deref(),
        Some(std::ffi::OsStr::new("/runtime/bin:/system/bin"))
    );
    fs::remove_dir_all(root).expect("test directory should be removed");
}

#[cfg(unix)]
#[test]
fn login_shell_path_times_out_and_reaps_stuck_shell() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        process::Command,
        time::{Duration, Instant},
    };
    use uuid::Uuid;

    let root = env::temp_dir().join(format!("sprite-studio-shell-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("test directory should exist");
    let shell = root.join("shell");
    fs::write(&shell, "#!/bin/sh\nwhile :; do :; done\n").expect("fake shell should be written");
    let mut permissions = fs::metadata(&shell)
        .expect("fake shell metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&shell, permissions).expect("fake shell should be executable");

    let started = Instant::now();
    let discovered = login_shell_path_with_timeout(&shell, Duration::from_millis(250));
    let elapsed = started.elapsed();

    assert!(discovered.is_none());
    assert!(elapsed < Duration::from_secs(1), "elapsed: {elapsed:?}");
    assert!(!Command::new("/usr/bin/pgrep")
        .args(["-f", shell.to_str().expect("UTF-8 test path")])
        .output()
        .expect("process probe should run")
        .status
        .success());
    fs::remove_dir_all(root).expect("test directory should be removed");
}

#[cfg(unix)]
#[test]
fn login_shell_timeout_kills_descendants() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::Duration};
    use uuid::Uuid;

    let root = env::temp_dir().join(format!("sprite-studio-shell-tree-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("test directory should exist");
    let descendant_pid = root.join("descendant.pid");
    let shell = root.join("shell");
    fs::write(
            &shell,
            format!(
                "#!/bin/sh\n(sleep 30) >/dev/null 2>&1 &\nprintf '%s' \"$!\" > '{}'\nwhile :; do :; done\n",
                descendant_pid.display()
            ),
        )
        .expect("fake shell should be written");
    let mut permissions = fs::metadata(&shell)
        .expect("fake shell metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&shell, permissions).expect("fake shell should be executable");

    let discovered = login_shell_path_with_timeout(&shell, Duration::from_secs(2));
    let pid = fs::read_to_string(&descendant_pid).expect("descendant should record its pid");
    let descendant_survived = Command::new("/bin/kill")
        .args(["-0", pid.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("descendant probe should run")
        .success();
    if descendant_survived {
        let _ = Command::new("/bin/kill")
            .args(["-KILL", pid.trim()])
            .status();
    }
    fs::remove_dir_all(root).expect("test directory should be removed");

    assert!(discovered.is_none());
    assert!(!descendant_survived, "timed-out shell left a descendant");
}

#[cfg(unix)]
#[test]
fn login_shell_output_is_bounded_when_descendants_inherit_handles() {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        time::{Duration, Instant},
    };
    use uuid::Uuid;

    let root = env::temp_dir().join(format!("sprite-studio-shell-output-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("test directory should exist");
    let descendant_pid = root.join("descendant.pid");
    let shell = root.join("shell");
    fs::write(
            &shell,
            format!(
                "#!/bin/sh\n(sleep 5) &\nprintf '%s' \"$!\" > '{}'\nprintf '\\n__SPRITE_STUDIO_PATH__=/runtime/bin:/system/bin\\n'\n",
                descendant_pid.display()
            ),
        )
        .expect("fake shell should be written");
    let mut permissions = fs::metadata(&shell)
        .expect("fake shell metadata")
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&shell, permissions).expect("fake shell should be executable");

    let started = Instant::now();
    let discovered = login_shell_path_with_timeout(&shell, Duration::from_secs(3));
    let elapsed = started.elapsed();
    let pid = fs::read_to_string(&descendant_pid).expect("descendant should record its pid");
    let descendant_survived = std::process::Command::new("/bin/kill")
        .args(["-0", pid.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .expect("descendant probe should run")
        .success();
    if descendant_survived {
        let _ = std::process::Command::new("/bin/kill")
            .args(["-KILL", pid.trim()])
            .status();
    }
    fs::remove_dir_all(root).expect("test directory should be removed");

    assert_eq!(
        discovered.as_deref(),
        Some(std::ffi::OsStr::new("/runtime/bin:/system/bin"))
    );
    assert!(elapsed < Duration::from_secs(4), "elapsed: {elapsed:?}");
    assert!(!descendant_survived, "successful shell left a descendant");
}

#[cfg(unix)]
#[test]
fn provider_environment_runs_env_shebang_with_shell_runtime() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    use uuid::Uuid;

    let root = env::temp_dir().join(format!("sprite-studio-provider-test-{}", Uuid::new_v4()));
    let runtime_bin = root.join("runtime/bin");
    fs::create_dir_all(&runtime_bin).expect("runtime directory should exist");

    let node = runtime_bin.join("node");
    fs::write(&node, "#!/bin/sh\nexit 0\n").expect("fake node should be written");
    let provider = root.join("codex");
    fs::write(&provider, "#!/usr/bin/env node\n").expect("fake provider should be written");
    let shell = root.join("shell");
    fs::write(
        &shell,
        format!(
            "#!/bin/sh\nprintf '__SPRITE_STUDIO_PATH__={}:{}\\n'\n",
            runtime_bin.display(),
            "/usr/bin:/bin"
        ),
    )
    .expect("fake shell should be written");

    for executable in [&node, &provider, &shell] {
        let mut permissions = fs::metadata(executable)
            .expect("executable metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(executable, permissions).expect("file should be executable");
    }

    let inherited = env::join_paths([PathBuf::from("/usr/bin"), PathBuf::from("/bin")])
        .expect("valid restricted PATH");
    let path = provider_environment_path(Some(inherited.as_os_str()), Some(&shell), Some(&root));
    let status = Command::new(&provider)
        .env("PATH", path)
        .status()
        .expect("provider should start");

    assert!(status.success());
    fs::remove_dir_all(root).expect("test directory should be removed");
}

#[test]
fn merges_login_shell_path_for_gui_provider_processes() {
    let runtime_bin = PathBuf::from("/runtime/bin");
    let system_bin = PathBuf::from("/system/bin");
    let inherited = env::join_paths([system_bin.clone()]).expect("valid inherited PATH");
    let login_shell =
        env::join_paths([runtime_bin.clone(), system_bin.clone()]).expect("valid shell PATH");
    let home = PathBuf::from("/Users/example");

    let merged = merge_provider_paths(
        Some(inherited.as_os_str()),
        Some(login_shell.as_os_str()),
        Some(&home),
    );
    let entries = env::split_paths(&merged).collect::<Vec<_>>();

    assert_eq!(entries.first(), Some(&runtime_bin));
    assert_eq!(
        entries.iter().filter(|path| **path == system_bin).count(),
        1
    );
    assert!(entries.contains(&home.join(".local/bin")));
}

#[test]
fn parses_antigravity_plaintext_models_table() {
    let modes = antigravity_modes_from_text(
        "gemini-3.8-flash-high     Gemini 3.8 Flash (High)\n\
             gemini-3.1-pro-high       Gemini 3.1 Pro (High)\n\
             claude-sonnet-4-6         Claude Sonnet 4.6 (Thinking)\n",
    );
    assert_eq!(modes.len(), 3);
    assert_eq!(modes[0].id, "gemini-3.8-flash-high");
    assert_eq!(modes[0].label, "Gemini 3.8 Flash (High)");
    assert_eq!(modes[2].id, "claude-sonnet-4-6");
    assert!(modes[0].reasoning_efforts.contains(&"high".into()));
}

#[test]
fn cursor_image_stays_agent_native_only_for_cursor_chats() {
    assert!(!is_provider_native_image("cursor-image", "cursor"));
    assert!(is_provider_native_image("cursor-image", "claude"));
    assert!(!is_provider_native_image("imagegen", "codex"));
    assert!(is_provider_native_image("imagegen", "cursor"));
    assert!(is_provider_native_image("provider-native", "cursor"));
    assert!(!is_provider_native_image(
        "antigravity-image",
        "antigravity"
    ));
    assert!(is_provider_native_image("antigravity-image", "claude"));
}

#[test]
fn windows_lookup_names_include_exe_suffix() {
    let names = executable_lookup_names("agent");
    assert!(names.iter().any(|name| name == "agent"));
    #[cfg(windows)]
    {
        assert!(names.iter().any(|name| name == "agent.exe"));
        assert!(names.iter().any(|name| name == "agent.cmd"));
    }
    #[cfg(not(windows))]
    assert!(!names.iter().any(|name| name.ends_with(".exe")));
}

#[test]
fn cursor_auth_from_status_json_respects_is_authenticated() {
    let unauthenticated = serde_json::json!({
        "status": "unauthenticated",
        "isAuthenticated": false,
        "message": "Not logged in"
    });
    assert_eq!(cursor_auth_from_status_json(&unauthenticated), Some(false));

    let authenticated = serde_json::json!({
        "status": "authenticated",
        "isAuthenticated": true,
        "hasAccessToken": true
    });
    assert_eq!(cursor_auth_from_status_json(&authenticated), Some(true));
}

#[test]
fn cursor_install_command_uses_official_installer() {
    let command = cursor_cli_install_command();
    #[cfg(windows)]
    {
        assert_eq!(command.get_program().to_string_lossy(), "powershell");
        let script = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(script.contains("cursor.com/install?win32=true"));
    }
    #[cfg(not(windows))]
    {
        assert_eq!(command.get_program().to_string_lossy(), "bash");
        let script = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(script.contains("cursor.com/install"));
    }
}

#[test]
fn headless_provider_flags_do_not_panic() {
    let _path = provider_process_path("cursor");
    let mut command = std::process::Command::new("cmd");
    apply_std_headless_flags(&mut command);
}

#[cfg(windows)]
#[test]
fn cursor_process_path_puts_git_bash_ahead_of_system32_when_installed() {
    let cursor = provider_process_path("cursor");
    let other = provider_process_path("codex");
    let cursor_entries: Vec<_> = env::split_paths(&cursor).collect();
    let other_entries: Vec<_> = env::split_paths(&other).collect();
    let git_bin = [env::var_os("ProgramW6432"), env::var_os("ProgramFiles")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .map(|root| root.join("Git").join("bin"))
        .find(|path| path.is_dir());
    if let Some(git_bin) = git_bin {
        let git_index = cursor_entries
            .iter()
            .position(|path| path == &git_bin)
            .expect("Git bin should be on the Cursor PATH");
        let system32 = PathBuf::from(r"C:\Windows\System32");
        if let Some(system_index) = cursor_entries.iter().position(|path| path == &system32) {
            assert!(git_index < system_index);
        }
        assert_ne!(cursor_entries.first(), other_entries.first());
    }
}

#[test]
fn antigravity_install_command_uses_official_installer() {
    let command = antigravity_cli_install_command();
    #[cfg(windows)]
    {
        assert_eq!(command.get_program().to_string_lossy(), "powershell");
        let script = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(script.contains("antigravity.google/cli/install.ps1"));
    }
    #[cfg(not(windows))]
    {
        assert_eq!(command.get_program().to_string_lossy(), "bash");
        let script = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(script.contains("antigravity.google/cli/install.sh"));
    }
}
