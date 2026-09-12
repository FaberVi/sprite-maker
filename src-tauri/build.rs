fn main() {
    tauri_build::build();
    request_common_controls_v6();
}

/// Windows lib tests do not get Tauri's GUI manifest. Without Common Controls v6,
/// `rfd`/`tao` import `TaskDialogIndirect` from comctl32 v5 and the test exe fails
/// to start with STATUS_ENTRYPOINT_NOT_FOUND.
fn request_common_controls_v6() {
    #[cfg(windows)]
    {
        println!("cargo:rerun-if-changed=build.rs");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
}
