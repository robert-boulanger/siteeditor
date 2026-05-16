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
pub struct AssetInfo {
    /// relativer Pfad unter `assets/`, mit `/` als Trenner
    pub path: String,
    pub name: String,
    pub size: u64,
    pub mime: String,
    pub mtime: u64,
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

    /// Schreibt Frontmatter UND Body neu. Verwendet, wenn Blocks oder
    /// Metadaten geändert wurden. Frontmatter wird aus dem strukturierten
    /// `PageFrontmatter` serialisiert; Reihenfolge der YAML-Felder folgt
    /// damit der Struct-Definition (nicht byte-genau zur Vorversion).
    pub fn save_page_full(
        &self,
        slug: &str,
        frontmatter: &PageFrontmatter,
        new_body: &str,
    ) -> Result<(), ProjectError> {
        if !is_safe_slug(slug) {
            return Err(ProjectError::InvalidPage {
                path: slug.to_string(),
                reason: "slug enthält unerlaubte Zeichen".into(),
            });
        }
        let path = self.pages_dir().join(format!("{slug}.md"));
        if !path.exists() {
            return Err(ProjectError::InvalidPage {
                path: path.to_string(),
                reason: "page existiert nicht — erst create_page aufrufen".into(),
            });
        }
        let raw = std::fs::read_to_string(&path)?;
        let fm_yaml = serde_yaml::to_string(frontmatter).map_err(|e| ProjectError::InvalidPage {
            path: path.to_string(),
            reason: format!("frontmatter serialise: {e}"),
        })?;
        let assembled = assemble_page(fm_yaml.trim_end_matches('\n'), new_body);

        backup_file(&self.root, slug, &raw)?;
        atomic_write(&path, &assembled)?;
        Ok(())
    }

    /// Legt eine neue Page an. Schlägt fehl, wenn `slug` bereits existiert.
    pub fn create_page(
        &self,
        slug: &str,
        frontmatter: &PageFrontmatter,
        body: &str,
    ) -> Result<(), ProjectError> {
        if !is_safe_slug(slug) {
            return Err(ProjectError::InvalidPage {
                path: slug.to_string(),
                reason: "slug enthält unerlaubte Zeichen".into(),
            });
        }
        let path = self.pages_dir().join(format!("{slug}.md"));
        if path.exists() {
            return Err(ProjectError::InvalidPage {
                path: path.to_string(),
                reason: "page existiert bereits".into(),
            });
        }
        let fm_yaml = serde_yaml::to_string(frontmatter).map_err(|e| ProjectError::InvalidPage {
            path: path.to_string(),
            reason: format!("frontmatter serialise: {e}"),
        })?;
        let assembled = assemble_page(fm_yaml.trim_end_matches('\n'), body);
        atomic_write(&path, &assembled)?;
        Ok(())
    }

    /// Benennt eine Page um. Schlägt fehl, wenn das Ziel existiert oder die
    /// Quelle fehlt. Backups des alten Slugs werden mit verschoben.
    pub fn rename_page(&self, old_slug: &str, new_slug: &str) -> Result<(), ProjectError> {
        if !is_safe_slug(new_slug) {
            return Err(ProjectError::InvalidPage {
                path: new_slug.to_string(),
                reason: "slug enthält unerlaubte Zeichen".into(),
            });
        }
        if old_slug == new_slug {
            return Ok(());
        }
        let old_path = self.pages_dir().join(format!("{old_slug}.md"));
        let new_path = self.pages_dir().join(format!("{new_slug}.md"));
        if !old_path.exists() {
            return Err(ProjectError::InvalidPage {
                path: old_path.to_string(),
                reason: "alte page existiert nicht".into(),
            });
        }
        if new_path.exists() {
            return Err(ProjectError::InvalidPage {
                path: new_path.to_string(),
                reason: "ziel-slug existiert bereits".into(),
            });
        }
        std::fs::rename(&old_path, &new_path)?;

        // Backups mitziehen
        let old_backup = self.root.join(".siteeditor/backups").join(old_slug);
        let new_backup = self.root.join(".siteeditor/backups").join(new_slug);
        if old_backup.exists() && !new_backup.exists() {
            std::fs::rename(&old_backup, &new_backup)?;
        }
        Ok(())
    }

    /// Listet alle Dateien in `<root>/assets/` (rekursiv, relative Pfade mit
    /// `/`-Trennern). Ordner selbst tauchen nicht in der Liste auf.
    pub fn list_assets(&self) -> Result<Vec<AssetInfo>, ProjectError> {
        let dir = self.assets_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in walkdir::WalkDir::new(&dir).min_depth(1) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let abs = Utf8Path::from_path(entry.path()).ok_or(ProjectError::NonUtf8Path)?;
            let rel = abs
                .strip_prefix(&dir)
                .map_err(|_| ProjectError::NonUtf8Path)?
                .as_str()
                .replace('\\', "/");
            let meta = entry.metadata()?;
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let name = abs.file_name().unwrap_or("").to_string();
            out.push(AssetInfo {
                path: rel,
                name,
                size: meta.len(),
                mime: guess_mime(abs.as_str()).to_string(),
                mtime,
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// Kopiert eine externe Datei nach `<root>/assets/`. Bei Namens-Kollision
    /// wird `name-1.ext`, `name-2.ext`, … vergeben. Gibt den relativen Pfad
    /// (mit `/`) zurück.
    pub fn import_asset(&self, source: impl AsRef<Utf8Path>) -> Result<String, ProjectError> {
        let source = source.as_ref();
        if !source.is_file() {
            return Err(ProjectError::InvalidPage {
                path: source.to_string(),
                reason: "quelle ist keine datei".into(),
            });
        }
        let dir = self.assets_dir();
        std::fs::create_dir_all(&dir)?;
        let original_name = source.file_name().unwrap_or("asset");
        let (stem, ext) = split_name(original_name);
        let mut candidate = original_name.to_string();
        let mut i: u32 = 1;
        while dir.join(&candidate).exists() {
            candidate = if ext.is_empty() {
                format!("{stem}-{i}")
            } else {
                format!("{stem}-{i}.{ext}")
            };
            i += 1;
        }
        let target = dir.join(&candidate);
        std::fs::copy(source, &target)?;
        Ok(candidate)
    }

    /// Löscht eine Datei in `assets/`. Lehnt jeden Pfad ab, der ausserhalb
    /// von `assets/` zeigen würde (`..`, absolut, Symlink-Escape).
    pub fn delete_asset(&self, rel_path: &str) -> Result<(), ProjectError> {
        if !is_safe_asset_path(rel_path) {
            return Err(ProjectError::InvalidPage {
                path: rel_path.to_string(),
                reason: "unsicherer asset-pfad".into(),
            });
        }
        let dir = self.assets_dir();
        let target = dir.join(rel_path);
        // canonicalize beider Pfade und prüfen, dass target unter dir liegt
        let dir_canon = std::fs::canonicalize(&dir)?;
        let target_canon = std::fs::canonicalize(&target)?;
        if !target_canon.starts_with(&dir_canon) {
            return Err(ProjectError::InvalidPage {
                path: rel_path.to_string(),
                reason: "pfad verlässt asset-verzeichnis".into(),
            });
        }
        std::fs::remove_file(&target_canon)?;
        Ok(())
    }

    /// Löscht eine Page. Backups verbleiben unter `.siteeditor/backups/<slug>/`
    /// zur Wiederherstellung.
    pub fn delete_page(&self, slug: &str) -> Result<(), ProjectError> {
        let path = self.pages_dir().join(format!("{slug}.md"));
        if !path.exists() {
            return Err(ProjectError::InvalidPage {
                path: path.to_string(),
                reason: "page existiert nicht".into(),
            });
        }
        // Letzte Version noch einmal sichern, dann löschen.
        if let Ok(raw) = std::fs::read_to_string(&path) {
            let _ = backup_file(&self.root, slug, &raw);
        }
        std::fs::remove_file(&path)?;
        Ok(())
    }
}

/// Slug-Sicherheit: keine Pfad-Trenner, keine `..`, kein Leerstring.
/// Strenge Slug-Regeln (Kebab-Case) prüft `theme_contract::is_valid_slug`.
fn is_safe_slug(slug: &str) -> bool {
    !slug.is_empty()
        && !slug.contains('/')
        && !slug.contains('\\')
        && !slug.contains("..")
        && !slug.starts_with('.')
}

fn is_safe_asset_path(p: &str) -> bool {
    if p.is_empty() {
        return false;
    }
    if p.starts_with('/') || p.starts_with('\\') {
        return false;
    }
    // Windows-Laufwerksbuchstaben & UNC
    if p.contains(':') {
        return false;
    }
    for seg in p.split(['/', '\\']) {
        if seg.is_empty() || seg == ".." || seg == "." {
            return false;
        }
    }
    true
}

fn split_name(name: &str) -> (String, String) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), ext.to_string()),
        _ => (name.to_string(), String::new()),
    }
}

fn guess_mime(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "pdf" => "application/pdf",
        "txt" | "md" => "text/plain",
        "json" => "application/json",
        _ => "application/octet-stream",
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

    fn make_project_with_page(page_md: &str) -> (tempfile::TempDir, Project) {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(tmp.path()).unwrap().to_path_buf();
        std::fs::write(
            root.join("site.json"),
            r#"{"schema_version":"0.2","title":"T","base_url":"https://x","active_theme":"default"}"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("pages")).unwrap();
        std::fs::write(root.join("pages/index.md"), page_md).unwrap();
        let project = Project::open(&root).unwrap();
        (tmp, project)
    }

    // --- save_page_full / create / rename / delete --------------------------

    fn fm(title: &str) -> PageFrontmatter {
        PageFrontmatter {
            title: title.into(),
            ..Default::default()
        }
    }

    #[test]
    fn save_page_full_is_atomic_no_tmp_left() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        project.save_page_full("index", &fm("X"), "").unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(project.pages_dir())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "tmp file leaked: {leftovers:?}");
    }

    #[test]
    fn save_page_full_rotates_backups_keeping_last_10() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        let backup_dir = project.root.join(".siteeditor/backups/index");
        std::fs::create_dir_all(&backup_dir).unwrap();
        for i in 0..15u32 {
            std::fs::write(backup_dir.join(format!("{i:010}.md")), format!("old-{i}")).unwrap();
        }
        project.save_page_full("index", &fm("X"), "").unwrap();
        let count = std::fs::read_dir(&backup_dir).unwrap().count();
        assert!(count <= 10, "backup rotation kept {count} files (>10)");
        assert!(!backup_dir.join("0000000000.md").exists());
        assert!(backup_dir.join("0000000014.md").exists());
    }

    #[test]
    fn save_page_full_rewrites_frontmatter_and_body() {
        let (_tmp, project) = make_project_with_page("---\ntitle: Old\n---\nold body\n");
        let mut new_fm = fm("New Title");
        new_fm.visible = false;
        new_fm.blocks = vec![serde_json::json!({"type": "text"})];

        project.save_page_full("index", &new_fm, "new body\n").unwrap();

        let reloaded = project.load_page("index").unwrap();
        assert_eq!(reloaded.frontmatter.title, "New Title");
        assert!(!reloaded.frontmatter.visible);
        assert_eq!(reloaded.frontmatter.blocks.len(), 1);
        assert_eq!(reloaded.body_markdown, "new body\n");
    }

    #[test]
    fn save_page_full_rejects_missing_page() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\nbody\n");
        let err = project.save_page_full("ghost", &fm("X"), "").unwrap_err();
        match err {
            ProjectError::InvalidPage { reason, .. } => assert!(reason.contains("existiert nicht")),
            other => panic!("unerwarteter Fehler: {other:?}"),
        }
    }

    #[test]
    fn save_page_full_rejects_unsafe_slug() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\nbody\n");
        for bad in ["../etc", "a/b", "..", ".hidden", ""] {
            let err = project.save_page_full(bad, &fm("X"), "").unwrap_err();
            assert!(matches!(err, ProjectError::InvalidPage { .. }), "slug {bad} sollte abgelehnt werden");
        }
    }

    #[test]
    fn create_page_writes_and_rejects_existing() {
        let (_tmp, project) = make_project_with_page("---\ntitle: Home\n---\nbody\n");
        project.create_page("about", &fm("Über uns"), "Hallo\n").unwrap();
        let p = project.load_page("about").unwrap();
        assert_eq!(p.frontmatter.title, "Über uns");
        assert_eq!(p.body_markdown, "Hallo\n");

        let err = project.create_page("about", &fm("X"), "").unwrap_err();
        match err {
            ProjectError::InvalidPage { reason, .. } => assert!(reason.contains("existiert bereits")),
            other => panic!("unerwarteter Fehler: {other:?}"),
        }
    }

    #[test]
    fn rename_page_moves_file_and_backups() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        // einen Backup anlegen (save_page_full → backup_file)
        project.save_page_full("index", &fm("T2"), "").unwrap();
        let old_backup_dir = project.root.join(".siteeditor/backups/index");
        assert!(old_backup_dir.exists());

        project.rename_page("index", "home").unwrap();
        assert!(!project.pages_dir().join("index.md").exists());
        assert!(project.pages_dir().join("home.md").exists());
        assert!(!old_backup_dir.exists());
        assert!(project.root.join(".siteeditor/backups/home").exists());
    }

    #[test]
    fn rename_page_rejects_target_collision() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\nbody\n");
        project.create_page("about", &fm("A"), "").unwrap();
        let err = project.rename_page("index", "about").unwrap_err();
        match err {
            ProjectError::InvalidPage { reason, .. } => assert!(reason.contains("existiert bereits")),
            other => panic!("unerwarteter Fehler: {other:?}"),
        }
    }

    #[test]
    fn rename_page_same_slug_is_noop() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\nbody\n");
        project.rename_page("index", "index").unwrap();
        assert!(project.pages_dir().join("index.md").exists());
    }

    // --- assets --------------------------------------------------------------

    fn write_asset(project: &Project, rel: &str, content: &[u8]) {
        let target = project.assets_dir().join(rel);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(target, content).unwrap();
    }

    #[test]
    fn list_assets_returns_relative_paths_sorted() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        write_asset(&project, "b.png", b"PNG");
        write_asset(&project, "sub/a.jpg", b"JPG");
        let assets = project.list_assets().unwrap();
        let paths: Vec<_> = assets.iter().map(|a| a.path.clone()).collect();
        assert_eq!(paths, vec!["b.png".to_string(), "sub/a.jpg".to_string()]);
        let png = assets.iter().find(|a| a.path == "b.png").unwrap();
        assert_eq!(png.mime, "image/png");
        assert_eq!(png.size, 3);
    }

    #[test]
    fn list_assets_empty_when_dir_missing() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        assert!(project.list_assets().unwrap().is_empty());
    }

    #[test]
    fn import_asset_copies_and_returns_name() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("logo.png");
        std::fs::write(&src, b"DATA").unwrap();
        let rel = project.import_asset(Utf8Path::from_path(&src).unwrap()).unwrap();
        assert_eq!(rel, "logo.png");
        assert!(project.assets_dir().join("logo.png").exists());
    }

    #[test]
    fn import_asset_renames_on_conflict() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        write_asset(&project, "logo.png", b"OLD");
        write_asset(&project, "logo-1.png", b"OLD1");
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("logo.png");
        std::fs::write(&src, b"NEW").unwrap();
        let rel = project.import_asset(Utf8Path::from_path(&src).unwrap()).unwrap();
        assert_eq!(rel, "logo-2.png");
        assert_eq!(std::fs::read(project.assets_dir().join("logo-2.png")).unwrap(), b"NEW");
        assert_eq!(std::fs::read(project.assets_dir().join("logo.png")).unwrap(), b"OLD");
    }

    #[test]
    fn import_asset_handles_no_extension() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        write_asset(&project, "README", b"OLD");
        let src_dir = tempfile::tempdir().unwrap();
        let src = src_dir.path().join("README");
        std::fs::write(&src, b"NEW").unwrap();
        let rel = project.import_asset(Utf8Path::from_path(&src).unwrap()).unwrap();
        assert_eq!(rel, "README-1");
    }

    #[test]
    fn delete_asset_removes_file() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        write_asset(&project, "logo.png", b"X");
        project.delete_asset("logo.png").unwrap();
        assert!(!project.assets_dir().join("logo.png").exists());
    }

    #[test]
    fn delete_asset_rejects_escape() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        for bad in ["../site.json", "../../etc/passwd", "/etc/passwd", "", "a/../b"] {
            let err = project.delete_asset(bad).unwrap_err();
            assert!(matches!(err, ProjectError::InvalidPage { .. }), "{bad} sollte abgelehnt werden");
        }
    }

    #[test]
    fn delete_asset_rejects_missing() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        // assets-dir muss existieren, sonst schlägt canonicalize bereits am dir fehl
        std::fs::create_dir_all(project.assets_dir()).unwrap();
        let err = project.delete_asset("ghost.png").unwrap_err();
        // entweder Io (canonicalize) oder InvalidPage — beide ok, file_not_found
        match err {
            ProjectError::Io(_) | ProjectError::InvalidPage { .. } => (),
            other => panic!("unerwarteter Fehler: {other:?}"),
        }
    }

    #[test]
    fn delete_page_removes_file_and_keeps_backup() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\nbody\n");
        project.delete_page("index").unwrap();
        assert!(!project.pages_dir().join("index.md").exists());
        // letzte Version liegt als Backup
        let backup_dir = project.root.join(".siteeditor/backups/index");
        assert!(backup_dir.exists());
        assert!(std::fs::read_dir(&backup_dir).unwrap().count() >= 1);

        let err = project.delete_page("index").unwrap_err();
        assert!(matches!(err, ProjectError::InvalidPage { .. }));
    }
}
