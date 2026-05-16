//! Site-Projekt auf Platte: lesen, schreiben.
//!
//! Phase 04 MVP — `open()`, `list_pages()`, `load_page()` für Smoke-Test.

use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("walk: {0}")]
    Walk(#[from] walkdir::Error),
    #[error("invalid site.json: {0}")]
    InvalidSiteJson(String),
    #[error("invalid page {path}: {reason}")]
    InvalidPage { path: String, reason: String },
    #[error("not a directory: {0}")]
    NotADirectory(Utf8PathBuf),
    #[error("missing site.json in {0}")]
    MissingSiteJson(Utf8PathBuf),
    #[error("non-utf8 path")]
    NonUtf8Path,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SiteManifest {
    pub schema_version: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub base_url: String,
    pub active_theme: String,
    #[serde(default)]
    pub default_template: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub menu_order: Vec<String>,
    #[serde(default)]
    pub css_var_overrides: BTreeMap<String, String>,
}

/// Frontmatter einer Page-Markdown-Datei.
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct PageFrontmatter {
    pub title: String,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default = "default_visible")]
    pub visible: bool,
    #[serde(default)]
    pub menu: MenuConfig,
    #[serde(default)]
    pub blocks: Vec<serde_json::Value>,
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
}

fn default_visible() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct MenuConfig {
    #[serde(default)]
    pub show: bool,
    #[serde(default)]
    pub order: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PageDoc {
    pub slug: String,
    pub frontmatter: PageFrontmatter,
    pub body_markdown: String,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: Utf8PathBuf,
    pub manifest: SiteManifest,
}

impl Project {
    /// Lädt ein Projekt aus dem Root-Verzeichnis. Erwartet `site.json`.
    pub fn open(root: impl AsRef<Utf8Path>) -> Result<Self, ProjectError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(ProjectError::NotADirectory(root.to_path_buf()));
        }
        let site_path = root.join("site.json");
        if !site_path.exists() {
            return Err(ProjectError::MissingSiteJson(root.to_path_buf()));
        }
        let raw = std::fs::read_to_string(&site_path)?;
        let manifest: SiteManifest = serde_json::from_str(&raw)
            .map_err(|e| ProjectError::InvalidSiteJson(e.to_string()))?;
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }

    pub fn pages_dir(&self) -> Utf8PathBuf {
        self.root.join("pages")
    }
    pub fn themes_dir(&self) -> Utf8PathBuf {
        self.root.join("themes")
    }
    pub fn assets_dir(&self) -> Utf8PathBuf {
        self.root.join("assets")
    }
    pub fn build_dir(&self) -> Utf8PathBuf {
        self.root.join(".siteeditor").join("build")
    }
    pub fn active_theme_dir(&self) -> Utf8PathBuf {
        self.themes_dir().join(&self.manifest.active_theme)
    }

    pub fn list_pages(&self) -> Result<Vec<PageDoc>, ProjectError> {
        let dir = self.pages_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut pages = Vec::new();
        for entry in walkdir::WalkDir::new(&dir).min_depth(1).max_depth(1) {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let utf8 = Utf8Path::from_path(path).ok_or(ProjectError::NonUtf8Path)?;
            let slug = utf8
                .file_stem()
                .ok_or_else(|| ProjectError::InvalidPage {
                    path: utf8.to_string(),
                    reason: "no file stem".into(),
                })?
                .to_owned();
            pages.push(load_page_file(utf8, slug)?);
        }
        pages.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(pages)
    }

    pub fn load_page(&self, slug: &str) -> Result<PageDoc, ProjectError> {
        let path = self.pages_dir().join(format!("{slug}.md"));
        load_page_file(&path, slug.to_string())
    }

    /// Tauscht den Body einer existierenden Page aus, ohne Frontmatter zu
    /// verändern. Schreibt atomar (tmp + rename) und legt die Vorgängerversion
    /// als Backup ab.
    pub fn save_page_body(&self, slug: &str, new_body: &str) -> Result<(), ProjectError> {
        let path = self.pages_dir().join(format!("{slug}.md"));
        let raw = std::fs::read_to_string(&path)?;
        let (frontmatter, _old_body) = split_frontmatter(&raw).map_err(|e| {
            ProjectError::InvalidPage {
                path: path.to_string(),
                reason: e,
            }
        })?;
        let assembled = assemble_page(&frontmatter, new_body);

        backup_file(&self.root, slug, &raw)?;
        atomic_write(&path, &assembled)?;
        Ok(())
    }
}

fn assemble_page(frontmatter: &str, body: &str) -> String {
    let mut s = String::with_capacity(frontmatter.len() + body.len() + 16);
    s.push_str("---\n");
    s.push_str(frontmatter);
    if !frontmatter.ends_with('\n') {
        s.push('\n');
    }
    s.push_str("---\n");
    s.push_str(body);
    s
}

fn atomic_write(target: &Utf8Path, content: &str) -> Result<(), ProjectError> {
    let parent = target.parent().ok_or_else(|| ProjectError::InvalidPage {
        path: target.to_string(),
        reason: "no parent dir".into(),
    })?;
    std::fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(
        ".{}.tmp",
        target.file_name().unwrap_or("page")
    ));
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, target)?;
    Ok(())
}

fn backup_file(root: &Utf8Path, slug: &str, content: &str) -> Result<(), ProjectError> {
    let dir = root.join(".siteeditor").join("backups").join(slug);
    std::fs::create_dir_all(&dir)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file = dir.join(format!("{ts}.md"));
    std::fs::write(&file, content)?;
    // keep last 10
    let mut entries: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("md"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    while entries.len() > 10 {
        let drop = entries.remove(0);
        let _ = std::fs::remove_file(drop.path());
    }
    Ok(())
}

fn load_page_file(path: &Utf8Path, slug: String) -> Result<PageDoc, ProjectError> {
    let raw = std::fs::read_to_string(path)?;
    let (frontmatter, body) = split_frontmatter(&raw).map_err(|e| ProjectError::InvalidPage {
        path: path.to_string(),
        reason: e,
    })?;
    let fm: PageFrontmatter = serde_yaml::from_str(&frontmatter).map_err(|e| {
        ProjectError::InvalidPage {
            path: path.to_string(),
            reason: format!("frontmatter yaml: {e}"),
        }
    })?;
    Ok(PageDoc {
        slug,
        frontmatter: fm,
        body_markdown: body,
    })
}

/// Splittet `---\n<yaml>\n---\n<body>`. Leerer Frontmatter erlaubt (`---\n---\n`).
fn split_frontmatter(s: &str) -> Result<(String, String), String> {
    let s = s.strip_prefix('\u{FEFF}').unwrap_or(s);
    let rest = s
        .strip_prefix("---\n")
        .or_else(|| s.strip_prefix("---\r\n"))
        .ok_or_else(|| "missing leading `---` frontmatter delimiter".to_string())?;
    // Find the closing `---` on its own line.
    let mut fm_end = None;
    let mut idx = 0usize;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            fm_end = Some((idx, idx + line.len()));
            break;
        }
        idx += line.len();
    }
    let (fm_end, body_start) =
        fm_end.ok_or_else(|| "missing closing `---` frontmatter delimiter".to_string())?;
    let frontmatter = rest[..fm_end].to_string();
    let body = rest[body_start..].to_string();
    Ok((frontmatter, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter_and_body() {
        let s = "---\ntitle: Hello\nvisible: true\n---\n# Body\n";
        let (fm, body) = split_frontmatter(s).unwrap();
        assert!(fm.contains("title: Hello"));
        assert_eq!(body, "# Body\n");
    }
}
