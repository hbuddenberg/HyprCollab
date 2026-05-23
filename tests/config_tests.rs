use hyprcollab::storage::{Config, GeneralConfig, KeysConfig, RagConfig, UiConfig, WaybarConfig};

// ═══════════════════════════════════════════
// Config defaults tests
// ═══════════════════════════════════════════

#[test]
fn test_config_default() {
    let config = Config::default();
    assert_eq!(config.general.language, "en");
    assert_eq!(config.general.default_agent, "openai");
    assert_eq!(config.ui.theme, "console");
    assert_eq!(config.ui.font_size, 13);
    assert!((config.ui.opacity - 0.95).abs() < f64::EPSILON);
    assert!(config.waybar.enabled);
    assert_eq!(config.waybar.refresh_interval, 1);
    assert_eq!(config.rag.chunk_size, 500);
    assert_eq!(config.rag.chunk_overlap, 50);
    assert_eq!(config.rag.top_k, 5);
    assert_eq!(config.keys.toggle, "SUPER+ALT+A");
    assert_eq!(config.keys.menu, "SUPER+ALT+M");
}

#[test]
fn test_config_default_agents() {
    let config = Config::default();
    assert_eq!(config.agents.len(), 1);
    assert_eq!(config.agents[0].name, "openai");
    assert_eq!(config.agents[0].agent_type, "openai");
    assert_eq!(
        config.agents[0].base_url,
        Some("http://localhost:11434/v1".to_string())
    );
}

// ═══════════════════════════════════════════
// Sub-config defaults tests
// ═══════════════════════════════════════════

#[test]
fn test_general_config_default() {
    let gc = GeneralConfig::default();
    assert_eq!(gc.language, "en");
    assert_eq!(gc.default_agent, "openai");
}

#[test]
fn test_ui_config_default() {
    let ui = UiConfig::default();
    assert_eq!(ui.theme, "console");
    assert_eq!(ui.font_size, 13);
    assert!((ui.opacity - 0.95).abs() < f64::EPSILON);
}

#[test]
fn test_waybar_config_default() {
    let wb = WaybarConfig::default();
    assert!(wb.enabled);
    assert_eq!(wb.refresh_interval, 1);
}

#[test]
fn test_rag_config_default() {
    let rag = RagConfig::default();
    assert_eq!(rag.chunk_size, 500);
    assert_eq!(rag.chunk_overlap, 50);
    assert_eq!(rag.top_k, 5);
    assert_eq!(rag.embedder, "text-embedding-3-small");
}

#[test]
fn test_keys_config_default() {
    let keys = KeysConfig::default();
    assert_eq!(keys.toggle, "SUPER+ALT+A");
    assert_eq!(keys.menu, "SUPER+ALT+M");
}

// ═══════════════════════════════════════════
// Serialization tests
// ═══════════════════════════════════════════

#[test]
fn test_config_yaml_roundtrip() {
    let config = Config::default();
    let yaml = serde_yaml::to_string(&config).unwrap();
    let back: Config = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config.general.language, back.general.language);
    assert_eq!(config.general.default_agent, back.general.default_agent);
    assert_eq!(config.ui.theme, back.ui.theme);
    assert_eq!(config.ui.font_size, back.ui.font_size);
    assert_eq!(config.agents.len(), back.agents.len());
}

#[test]
fn test_config_json_roundtrip() {
    let config = Config::default();
    let json = serde_json::to_string(&config).unwrap();
    let back: Config = serde_json::from_str(&json).unwrap();
    assert_eq!(config.general.language, back.general.language);
    assert_eq!(config.waybar.enabled, back.waybar.enabled);
    assert_eq!(config.rag.top_k, back.rag.top_k);
}

#[test]
fn test_config_yaml_contains_expected_keys() {
    let config = Config::default();
    let yaml = serde_yaml::to_string(&config).unwrap();
    assert!(yaml.contains("general"));
    assert!(yaml.contains("agents"));
    assert!(yaml.contains("ui"));
    assert!(yaml.contains("waybar"));
    assert!(yaml.contains("rag"));
    assert!(yaml.contains("keys"));
}

#[test]
fn test_ui_config_serialization() {
    let ui = UiConfig::default();
    let json = serde_json::to_string(&ui).unwrap();
    assert!(json.contains("console"));
    assert!(json.contains("13"));
}

#[test]
fn test_rag_config_serialization() {
    let rag = RagConfig::default();
    let json = serde_json::to_string(&rag).unwrap();
    assert!(json.contains("text-embedding-3-small"));
}
