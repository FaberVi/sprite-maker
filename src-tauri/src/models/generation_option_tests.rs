use super::GenerationOptions;

#[test]
fn omitted_frame_policy_defaults_to_full_auto_range() {
    let options: GenerationOptions =
        serde_json::from_str(r#"{"quality":"mid","width":64,"height":64,"frames":6,"fps":8}"#)
            .expect("generation options should deserialize");

    assert_eq!(options.frame_mode, "auto");
    assert_eq!(options.min_frames, 8);
    assert_eq!(options.max_frames, 12);
    assert!(options.allow_auto_adjust);
    assert!(!options.allow_interpolation);
}
