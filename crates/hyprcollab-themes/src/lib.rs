//! Theme engine — built-in and custom UI themes with CSS variable export.
//!
//! Implements the F5 S22 sprint item: 5 built-in colour schemes, YAML-based
//! custom themes stored in `~/.config/hyprcollab/themes/`, and CSS-variable
//! export.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    #[error("theme '{0}' not found")]
    NotFound(String),

    #[error("theme '{0}' is a built-in and cannot be deleted")]
    BuiltIn(String),

    #[error("theme validation failed: {0}")]
    Validation(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub type Result<T> = std::result::Result<T, ThemeError>;

// ── ThemeColors ───────────────────────────────────────────────────────────────

/// Full colour palette for a theme.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeColors {
    pub bg: String,
    pub fg: String,
    pub accent: String,
    pub surface: String,
    pub border: String,
    #[serde(default)]
    pub muted: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub success: Option<String>,
    #[serde(default)]
    pub warning: Option<String>,
}

impl ThemeColors {
    fn validate(&self) -> Result<()> {
        for (name, val) in [
            ("bg", &self.bg),
            ("fg", &self.fg),
            ("accent", &self.accent),
            ("surface", &self.surface),
            ("border", &self.border),
        ] {
            validate_hex(name, val)?;
        }
        Ok(())
    }
}

fn validate_hex(field: &str, value: &str) -> Result<()> {
    if !value.starts_with('#') || (value.len() != 7 && value.len() != 4) {
        return Err(ThemeError::Validation(format!(
            "field '{field}' must be a hex colour like #rrggbb, got '{value}'"
        )));
    }
    Ok(())
}

// ── Theme ─────────────────────────────────────────────────────────────────────

/// A complete UI theme descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub colors: ThemeColors,
    #[serde(default = "default_font")]
    pub font: String,
    #[serde(default = "default_font_size")]
    pub font_size: u32,
    #[serde(default = "default_border_radius")]
    pub border_radius: u32,
    #[serde(default)]
    pub custom_css: Option<String>,
}

fn default_font() -> String {
    "monospace".to_string()
}
fn default_font_size() -> u32 {
    14
}
fn default_border_radius() -> u32 {
    4
}

impl Theme {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(ThemeError::Validation("name must not be empty".into()));
        }
        self.colors.validate()
    }
}

// ── Built-in themes ───────────────────────────────────────────────────────────

fn builtin_themes() -> HashMap<String, Theme> {
    [
        Theme {
            name: "terminal-dark".into(),
            colors: ThemeColors {
                bg: "#0d1117".into(),
                fg: "#e6edf3".into(),
                accent: "#58a6ff".into(),
                surface: "#161b22".into(),
                border: "#30363d".into(),
                muted: Some("#8b949e".into()),
                error: Some("#f85149".into()),
                success: Some("#3fb950".into()),
                warning: Some("#d29922".into()),
            },
            font: "monospace".into(),
            font_size: 14,
            border_radius: 4,
            custom_css: None,
        },
        Theme {
            name: "catppuccin-mocha".into(),
            colors: ThemeColors {
                bg: "#1e1e2e".into(),
                fg: "#cdd6f4".into(),
                accent: "#89b4fa".into(),
                surface: "#313244".into(),
                border: "#45475a".into(),
                muted: Some("#6c7086".into()),
                error: Some("#f38ba8".into()),
                success: Some("#a6e3a1".into()),
                warning: Some("#f9e2af".into()),
            },
            font: "monospace".into(),
            font_size: 14,
            border_radius: 8,
            custom_css: None,
        },
        Theme {
            name: "nord".into(),
            colors: ThemeColors {
                bg: "#2e3440".into(),
                fg: "#d8dee9".into(),
                accent: "#88c0d0".into(),
                surface: "#3b4252".into(),
                border: "#434c5e".into(),
                muted: Some("#616e88".into()),
                error: Some("#bf616a".into()),
                success: Some("#a3be8c".into()),
                warning: Some("#ebcb8b".into()),
            },
            font: "monospace".into(),
            font_size: 14,
            border_radius: 4,
            custom_css: None,
        },
        Theme {
            name: "dracula".into(),
            colors: ThemeColors {
                bg: "#282a36".into(),
                fg: "#f8f8f2".into(),
                accent: "#bd93f9".into(),
                surface: "#44475a".into(),
                border: "#6272a4".into(),
                muted: Some("#6272a4".into()),
                error: Some("#ff5555".into()),
                success: Some("#50fa7b".into()),
                warning: Some("#ffb86c".into()),
            },
            font: "monospace".into(),
            font_size: 14,
            border_radius: 4,
            custom_css: None,
        },
        Theme {
            name: "tokyo-night".into(),
            colors: ThemeColors {
                bg: "#1a1b26".into(),
                fg: "#a9b1d6".into(),
                accent: "#7aa2f7".into(),
                surface: "#1f2335".into(),
                border: "#292e42".into(),
                muted: Some("#565f89".into()),
                error: Some("#f7768e".into()),
                success: Some("#9ece6a".into()),
                warning: Some("#e0af68".into()),
            },
            font: "monospace".into(),
            font_size: 14,
            border_radius: 4,
            custom_css: None,
        },
    ]
    .into_iter()
    .map(|t| (t.name.clone(), t))
    .collect()
}

// ── ThemeEngine ───────────────────────────────────────────────────────────────

/// Loads built-in and custom themes, exports CSS variables.
pub struct ThemeEngine {
    builtins: HashMap<String, Theme>,
    /// Directory where custom `.yaml` theme files are stored.
    custom_dir: PathBuf,
}

impl ThemeEngine {
    /// Create an engine that stores custom themes in `~/.config/hyprcollab/themes/`.
    pub fn new() -> Self {
        Self { builtins: builtin_themes(), custom_dir: Self::custom_theme_dir() }
    }

    /// Create an engine that stores custom themes in `dir` (useful for tests).
    pub fn with_custom_dir(dir: PathBuf) -> Self {
        Self { builtins: builtin_themes(), custom_dir: dir }
    }

    /// Default custom theme directory: `~/.config/hyprcollab/themes/`.
    pub fn custom_theme_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".config").join("hyprcollab").join("themes")
    }

    /// Load a theme by name (built-in first, then custom YAML file).
    pub fn load_theme(&self, name: &str) -> Result<Theme> {
        if let Some(t) = self.builtins.get(name) {
            return Ok(t.clone());
        }

        let path = self.custom_dir.join(format!("{name}.yaml"));
        if !path.exists() {
            return Err(ThemeError::NotFound(name.to_string()));
        }

        let yaml = std::fs::read_to_string(&path)?;
        let theme: Theme = serde_yaml::from_str(&yaml)?;
        theme.validate()?;
        Ok(theme)
    }

    /// List all available theme names (built-in + custom files on disk).
    pub fn list_themes(&self) -> Vec<String> {
        let mut names: Vec<String> = self.builtins.keys().cloned().collect();

        if let Ok(entries) = std::fs::read_dir(&self.custom_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) == Some("yaml")
                    && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                    && !self.builtins.contains_key(stem)
                {
                    names.push(stem.to_string());
                }
            }
        }

        names.sort();
        names
    }

    /// Save a custom theme YAML to disk. Overwrites existing custom themes.
    pub fn save_custom(&self, theme: &Theme) -> Result<()> {
        theme.validate()?;
        std::fs::create_dir_all(&self.custom_dir)?;
        let path = self.custom_dir.join(format!("{}.yaml", theme.name));
        let yaml = serde_yaml::to_string(theme)?;
        std::fs::write(path, yaml)?;
        Ok(())
    }

    /// Delete a custom theme. Returns an error if the theme is a built-in.
    pub fn delete_custom(&self, name: &str) -> Result<()> {
        if self.builtins.contains_key(name) {
            return Err(ThemeError::BuiltIn(name.to_string()));
        }

        let path = self.custom_dir.join(format!("{name}.yaml"));
        if !path.exists() {
            return Err(ThemeError::NotFound(name.to_string()));
        }

        std::fs::remove_file(path)?;
        Ok(())
    }

    /// Generate a CSS `:root` block with `--hc-*` custom properties.
    pub fn export_css(theme: &Theme) -> String {
        let c = &theme.colors;
        let mut props = vec![
            format!("  --hc-bg: {};", c.bg),
            format!("  --hc-fg: {};", c.fg),
            format!("  --hc-accent: {};", c.accent),
            format!("  --hc-surface: {};", c.surface),
            format!("  --hc-border: {};", c.border),
            format!("  --hc-font: {};", theme.font),
            format!("  --hc-font-size: {}px;", theme.font_size),
            format!("  --hc-border-radius: {}px;", theme.border_radius),
        ];

        if let Some(muted) = &c.muted {
            props.push(format!("  --hc-muted: {muted};"));
        }
        if let Some(error) = &c.error {
            props.push(format!("  --hc-error: {error};"));
        }
        if let Some(success) = &c.success {
            props.push(format!("  --hc-success: {success};"));
        }
        if let Some(warning) = &c.warning {
            props.push(format!("  --hc-warning: {warning};"));
        }
        if let Some(css) = &theme.custom_css {
            props.push(format!("  /* custom */\n  {css}"));
        }

        format!(":root {{\n{}\n}}", props.join("\n"))
    }
}

impl Default for ThemeEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn engine() -> ThemeEngine {
        ThemeEngine::new()
    }

    // ── Built-in loading ──────────────────────────────────────────────────

    #[test]
    fn loads_all_builtin_themes() {
        let e = engine();
        for name in ["terminal-dark", "catppuccin-mocha", "nord", "dracula", "tokyo-night"] {
            let t = e.load_theme(name).expect(name);
            assert_eq!(t.name, name);
        }
    }

    #[test]
    fn load_unknown_theme_returns_not_found() {
        let e = engine();
        let err = e.load_theme("nonexistent-xyz").unwrap_err();
        assert!(matches!(err, ThemeError::NotFound(_)));
    }

    #[test]
    fn list_themes_includes_all_builtins() {
        let e = engine();
        let names = e.list_themes();
        for expected in ["terminal-dark", "catppuccin-mocha", "nord", "dracula", "tokyo-night"] {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn list_themes_is_sorted() {
        let e = engine();
        let names = e.list_themes();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    // ── CSS export ────────────────────────────────────────────────────────

    #[test]
    fn css_export_starts_with_root() {
        let e = engine();
        let t = e.load_theme("terminal-dark").unwrap();
        let css = ThemeEngine::export_css(&t);
        assert!(css.starts_with(":root {"));
        assert!(css.ends_with('}'));
    }

    #[test]
    fn css_export_contains_required_variables() {
        let e = engine();
        let t = e.load_theme("nord").unwrap();
        let css = ThemeEngine::export_css(&t);

        for var in ["--hc-bg", "--hc-fg", "--hc-accent", "--hc-surface", "--hc-border"] {
            assert!(css.contains(var), "missing {var}");
        }
    }

    #[test]
    fn css_export_contains_correct_colour_values() {
        let e = engine();
        let t = e.load_theme("dracula").unwrap();
        let css = ThemeEngine::export_css(&t);
        assert!(css.contains("#282a36"), "bg missing");
        assert!(css.contains("#bd93f9"), "accent missing");
    }

    #[test]
    fn css_export_includes_optional_colours() {
        let e = engine();
        let t = e.load_theme("catppuccin-mocha").unwrap();
        let css = ThemeEngine::export_css(&t);
        assert!(css.contains("--hc-error"));
        assert!(css.contains("--hc-success"));
        assert!(css.contains("--hc-warning"));
    }

    // ── Theme validation ──────────────────────────────────────────────────

    #[test]
    fn validation_rejects_bad_hex() {
        let bad = Theme {
            name: "bad".into(),
            colors: ThemeColors {
                bg: "notahex".into(),
                fg: "#ffffff".into(),
                accent: "#000000".into(),
                surface: "#111111".into(),
                border: "#222222".into(),
                muted: None,
                error: None,
                success: None,
                warning: None,
            },
            font: "mono".into(),
            font_size: 14,
            border_radius: 4,
            custom_css: None,
        };
        assert!(matches!(bad.validate(), Err(ThemeError::Validation(_))));
    }

    #[test]
    fn validation_rejects_empty_name() {
        let t = Theme {
            name: "".into(),
            colors: ThemeColors {
                bg: "#000000".into(),
                fg: "#ffffff".into(),
                accent: "#ff0000".into(),
                surface: "#111111".into(),
                border: "#222222".into(),
                muted: None,
                error: None,
                success: None,
                warning: None,
            },
            font: "mono".into(),
            font_size: 14,
            border_radius: 4,
            custom_css: None,
        };
        assert!(matches!(t.validate(), Err(ThemeError::Validation(_))));
    }

    // ── Custom themes (isolated tmp dir via with_custom_dir) ─────────────

    fn minimal_theme(name: &str) -> Theme {
        Theme {
            name: name.into(),
            colors: ThemeColors {
                bg: "#000000".into(),
                fg: "#ffffff".into(),
                accent: "#ff0000".into(),
                surface: "#111111".into(),
                border: "#222222".into(),
                muted: None,
                error: None,
                success: None,
                warning: None,
            },
            font: "mono".into(),
            font_size: 14,
            border_radius: 4,
            custom_css: None,
        }
    }

    #[test]
    fn save_and_load_custom_theme() {
        let dir = tempfile::tempdir().unwrap();
        let engine = ThemeEngine::with_custom_dir(dir.path().to_path_buf());

        let theme = Theme {
            name: "my-custom".into(),
            colors: ThemeColors {
                bg: "#112233".into(),
                fg: "#aabbcc".into(),
                accent: "#ff0077".into(),
                surface: "#223344".into(),
                border: "#334455".into(),
                muted: None,
                error: None,
                success: None,
                warning: None,
            },
            font: "sans-serif".into(),
            font_size: 16,
            border_radius: 8,
            custom_css: None,
        };

        engine.save_custom(&theme).unwrap();
        let loaded = engine.load_theme("my-custom").unwrap();
        assert_eq!(loaded.name, "my-custom");
        assert_eq!(loaded.colors.bg, "#112233");
    }

    #[test]
    fn list_themes_includes_custom_after_save() {
        let dir = tempfile::tempdir().unwrap();
        let engine = ThemeEngine::with_custom_dir(dir.path().to_path_buf());

        engine.save_custom(&minimal_theme("zz-custom-test")).unwrap();
        let names = engine.list_themes();
        assert!(names.contains(&"zz-custom-test".to_string()));
    }

    #[test]
    fn delete_custom_theme() {
        let dir = tempfile::tempdir().unwrap();
        let engine = ThemeEngine::with_custom_dir(dir.path().to_path_buf());

        engine.save_custom(&minimal_theme("to-delete")).unwrap();
        engine.delete_custom("to-delete").unwrap();
        assert!(matches!(engine.load_theme("to-delete").unwrap_err(), ThemeError::NotFound(_)));
    }

    #[test]
    fn delete_builtin_returns_error() {
        let engine = engine();
        let err = engine.delete_custom("nord").unwrap_err();
        assert!(matches!(err, ThemeError::BuiltIn(_)));
    }

    #[test]
    fn custom_theme_dir_default_uses_home_env() {
        // custom_theme_dir() is a pure fn of HOME — verify the path shape.
        let dir = ThemeEngine::custom_theme_dir();
        assert!(dir.ends_with(".config/hyprcollab/themes"));
    }
}
