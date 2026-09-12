use super::GenerationManifest;

#[test]
fn generation_manifest_accepts_current_and_legacy_timestamp_keys() {
    for timestamp_key in ["generatedAt", "generation_time", "generated_at"] {
        let json = format!(
            r#"{{"name":"walk","category":"characters","fps":8,"files":["assets/characters/walk_01.png"],"{timestamp_key}":"2026-08-09T12:24:00Z"}}"#
        );
        let manifest: GenerationManifest =
            serde_json::from_str(&json).expect("manifest timestamp key should parse");
        assert_eq!(manifest.generated_at, "2026-08-09T12:24:00Z");
    }
}
