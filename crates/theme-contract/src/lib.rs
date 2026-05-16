//! Theme-Kontrakt (THEME_SPEC v0.2).
//!
//! MVP-Validator: prüft Manifest + Pflicht-Templates. Tera-Subset und
//! Tag-Verbote folgen, sobald der AST-Check dazukommt.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SUPPORTED_SPEC_MAJOR: u32 = 0;

pub const REQUIRED_CSS_VARS: &[&str] = &[
    "--color-primary",
    "--color-bg",
    "--color-text",
    "--font-body",
    "--font-heading",
    "--radius",
    "--spacing-section",
];

pub const REQUIRED_TEMPLATES: &[&str] = &[
    "templates/page.html",
    "templates/index.html",
    "templates/404.html",
    "templates/partials/head.html",
    "templates/partials/menu.html",
];

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ThemeManifest {
    pub spec_version: String,
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub supported_blocks: Vec<String>,
    #[serde(default)]
    pub unsupported_blocks: Vec<String>,
    pub css_variables: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Clone, Default)]
pub struct ValidationReport {
    pub ok: bool,
    pub errors: Vec<ValidationIssue>,
    pub warnings: Vec<ValidationIssue>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ValidationIssue {
    pub code: String,
    pub path: Option<String>,
    pub message: String,
}

pub fn validate_theme(theme_dir: &Utf8Path) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let manifest_path = theme_dir.join("theme.json");
    let manifest: Option<ThemeManifest> = match std::fs::read_to_string(&manifest_path) {
        Err(_) => {
            errors.push(ValidationIssue {
                code: "MISSING_MANIFEST".into(),
                path: Some("theme.json".into()),
                message: "theme.json fehlt".into(),
            });
            None
        }
        Ok(raw) => match serde_json::from_str::<ThemeManifest>(&raw) {
            Ok(m) => Some(m),
            Err(e) => {
                errors.push(ValidationIssue {
                    code: "MISSING_MANIFEST".into(),
                    path: Some("theme.json".into()),
                    message: format!("theme.json nicht parsbar: {e}"),
                });
                None
            }
        },
    };

    if let Some(m) = &manifest {
        match m.spec_version.split('.').next().and_then(|s| s.parse::<u32>().ok()) {
            Some(maj) if maj == SUPPORTED_SPEC_MAJOR => {}
            _ => errors.push(ValidationIssue {
                code: "BAD_SPEC_VERSION".into(),
                path: Some("theme.json".into()),
                message: format!(
                    "spec_version {} nicht unterstützt (Major {SUPPORTED_SPEC_MAJOR}.x erwartet)",
                    m.spec_version
                ),
            }),
        }
        if !is_valid_theme_name(&m.name) {
            errors.push(ValidationIssue {
                code: "BAD_THEME_NAME".into(),
                path: Some("theme.json".into()),
                message: format!("name '{}' verletzt ^[a-z0-9-]+$", m.name),
            });
        }
        for var in REQUIRED_CSS_VARS {
            if !m.css_variables.contains_key(*var) {
                errors.push(ValidationIssue {
                    code: "MISSING_CSS_VAR".into(),
                    path: Some("theme.json".into()),
                    message: format!("Pflicht-Variable {var} fehlt"),
                });
            }
        }
    }

    for tmpl in REQUIRED_TEMPLATES {
        if !theme_dir.join(tmpl).exists() {
            errors.push(ValidationIssue {
                code: "MISSING_TEMPLATE".into(),
                path: Some((*tmpl).into()),
                message: format!("Pflicht-Template {tmpl} fehlt"),
            });
        }
    }

    if !theme_dir.join("styles/main.css").exists() {
        warnings.push(ValidationIssue {
            code: "MISSING_CSS".into(),
            path: Some("styles/main.css".into()),
            message: "styles/main.css fehlt — Theme rendert ungestyled".into(),
        });
    }

    ValidationReport {
        ok: errors.is_empty(),
        errors,
        warnings,
    }
}

pub fn is_valid_theme_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn is_valid_slug(slug: &str) -> bool {
    if slug.is_empty() || slug.len() > 64 {
        return false;
    }
    let mut last_dash = false;
    for (i, c) in slug.chars().enumerate() {
        let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        if !ok {
            return false;
        }
        if c == '-' {
            if i == 0 || last_dash {
                return false;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }
    }
    !last_dash
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slug_rules() {
        assert!(is_valid_slug("index"));
        assert!(is_valid_slug("about-us"));
        assert!(is_valid_slug("page-1"));
        assert!(!is_valid_slug("Index"));
        assert!(!is_valid_slug("-x"));
        assert!(!is_valid_slug("x-"));
        assert!(!is_valid_slug("x--y"));
        assert!(!is_valid_slug(""));
    }
    #[test]
    fn theme_name_rules() {
        assert!(is_valid_theme_name("default"));
        assert!(is_valid_theme_name("my-theme-2"));
        assert!(!is_valid_theme_name("MyTheme"));
        assert!(!is_valid_theme_name(""));
    }
}
