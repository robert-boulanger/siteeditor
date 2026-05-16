//! Site-Projekt auf Platte: lesen, schreiben.
//!
//! Phase 04 MVP — `open()`, `list_pages()`, `load_page()` für Smoke-Test.

use camino::{Utf8Path, Utf8PathBuf};
use deploy_contract::DeployProfile;
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
    /// Phase 10: Deployment-Profile (Staging/Prod/…). Credentials liegen
    /// NICHT hier, sondern im OS-Keystore (Service-Name siehe
    /// [`Project::keystore_service_for`]).
    #[serde(default)]
    pub deploy_profiles: Vec<DeployProfile>,
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
    /// Im Editor als „Favorit" markiert — wird in der Sidebar oben angepinnt.
    /// Hat keine Wirkung auf den gerenderten Output.
    #[serde(default)]
    pub favorite: bool,
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
pub struct ThemeInfo {
    /// Verzeichnisname unter `themes/` — gleichzeitig `active_theme`-Wert.
    pub slug: String,
    /// `display_name` aus `theme.json`, fallback = slug.
    pub display_name: String,
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

    /// Listet alle Themes unter `themes/` (jeweils ein Verzeichnis mit
    /// `theme.json`). Sortiert alphabetisch nach Slug; `display_name`
    /// fällt auf den Slug zurück, falls `theme.json` ihn nicht setzt
    /// oder unleserlich ist.
    pub fn list_installed_themes(&self) -> Result<Vec<ThemeInfo>, ProjectError> {
        let dir = self.themes_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let slug = match entry.file_name().into_string() {
                Ok(s) => s,
                Err(_) => continue,
            };
            if slug.starts_with('.') {
                continue;
            }
            let manifest_path = dir.join(&slug).join("theme.json");
            if !manifest_path.exists() {
                continue;
            }
            let display_name = std::fs::read_to_string(&manifest_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                .and_then(|v| v.get("display_name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| slug.clone());
            out.push(ThemeInfo { slug, display_name });
        }
        out.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(out)
    }

    /// Liest die `styles/main.css` eines installierten Themes.
    pub fn read_theme_css(&self, slug: &str) -> Result<String, ProjectError> {
        let path = self.theme_css_path(slug)?;
        Ok(std::fs::read_to_string(&path)?)
    }

    /// Überschreibt die `styles/main.css` eines installierten Themes
    /// (atomar, ohne Backup-Rotation — der User akzeptiert die Änderung
    /// bewusst, Git ist die Wahrheit). Erzwingt `\n` am Ende.
    pub fn write_theme_css(&self, slug: &str, content: &str) -> Result<(), ProjectError> {
        let path = self.theme_css_path(slug)?;
        let normalized = if content.ends_with('\n') {
            content.to_string()
        } else {
            format!("{content}\n")
        };
        atomic_write(&path, &normalized)
    }

    fn theme_css_path(&self, slug: &str) -> Result<Utf8PathBuf, ProjectError> {
        if !is_safe_slug(slug) {
            return Err(ProjectError::InvalidPage {
                path: slug.to_string(),
                reason: "theme-slug enthält unerlaubte Zeichen".into(),
            });
        }
        let path = self.themes_dir().join(slug).join("styles").join("main.css");
        if !path.exists() {
            return Err(ProjectError::InvalidPage {
                path: path.to_string(),
                reason: "theme oder styles/main.css fehlt".into(),
            });
        }
        Ok(path)
    }

    /// Aktiviert ein installiertes Theme: schreibt `site.json` neu (mit
    /// `active_theme=<slug>`) und aktualisiert das in-memory-Manifest.
    /// Schlägt fehl, wenn das Theme nicht installiert ist oder der Slug
    /// unsicher wäre.
    pub fn set_active_theme(&mut self, slug: &str) -> Result<(), ProjectError> {
        if !is_safe_slug(slug) {
            return Err(ProjectError::InvalidPage {
                path: slug.to_string(),
                reason: "theme-slug enthält unerlaubte Zeichen".into(),
            });
        }
        let theme_dir = self.themes_dir().join(slug);
        if !theme_dir.join("theme.json").exists() {
            return Err(ProjectError::InvalidPage {
                path: theme_dir.to_string(),
                reason: "theme ist nicht installiert (theme.json fehlt)".into(),
            });
        }
        let mut new_manifest = self.manifest.clone();
        new_manifest.active_theme = slug.to_string();
        self.persist_manifest(new_manifest)
    }

    /// Listet alle Pages rekursiv unter `pages/`. Der Slug entspricht dem
    /// relativen Pfad zur Page-Datei ohne `.md`, mit `/`-Trennern:
    /// `pages/about.md` → `about`, `pages/about/team.md` → `about/team`.
    pub fn list_pages(&self) -> Result<Vec<PageDoc>, ProjectError> {
        let dir = self.pages_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut pages = Vec::new();
        for entry in walkdir::WalkDir::new(&dir).min_depth(1) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let utf8 = Utf8Path::from_path(path).ok_or(ProjectError::NonUtf8Path)?;
            let rel = utf8
                .strip_prefix(&dir)
                .map_err(|_| ProjectError::NonUtf8Path)?;
            let slug = rel
                .as_str()
                .trim_end_matches(".md")
                .replace('\\', "/")
                .to_string();
            if slug.is_empty() {
                continue;
            }
            pages.push(load_page_file(utf8, slug)?);
        }
        pages.sort_by(|a, b| a.slug.cmp(&b.slug));
        Ok(pages)
    }

    pub fn load_page(&self, slug: &str) -> Result<PageDoc, ProjectError> {
        let path = self.page_path(slug);
        load_page_file(&path, slug.to_string())
    }

    /// Pfad einer Page-Datei aus ihrem Slug (`a/b` → `pages/a/b.md`).
    fn page_path(&self, slug: &str) -> Utf8PathBuf {
        self.pages_dir().join(format!("{slug}.md"))
    }

    /// Pfad eines Section-Verzeichnisses (`about` → `pages/about/`).
    /// Existiert nur, wenn die Section Kinder hat.
    fn section_dir(&self, slug: &str) -> Utf8PathBuf {
        self.pages_dir().join(slug)
    }

    /// Markiert/entmarkiert eine Page als Favorit. Schreibt die Page-Datei
    /// neu mit aktualisiertem Frontmatter.
    pub fn set_favorite(&self, slug: &str, favorite: bool) -> Result<(), ProjectError> {
        let mut page = self.load_page(slug)?;
        if page.frontmatter.favorite == favorite {
            return Ok(());
        }
        page.frontmatter.favorite = favorite;
        self.save_page_full(slug, &page.frontmatter, &page.body_markdown)
    }

    /// Verschiebt eine Page (Reparent + optionaler `menu.order`-Update) in
    /// einer logisch atomaren Operation.
    ///
    /// - `new_parent = None` → die Page wird Root-Page.
    /// - `new_parent = Some("about")` → die Page wird Kind von `about`.
    ///   Der eigene Slug-Tail bleibt: `team` → `about/team`.
    /// - `new_order = Some(n)` → setzt `menu.order = n`.
    /// - Cycle-Verbot: ein Slug darf nicht in einen seiner Nachfahren
    ///   verschoben werden.
    /// - Wenn der Parent gleich bleibt, wird nur `menu.order` aktualisiert
    ///   (kein File-Move). Liefert den (ggf. neuen) Slug zurück.
    pub fn move_page(
        &self,
        slug: &str,
        new_parent: Option<&str>,
        new_order: Option<i32>,
    ) -> Result<String, ProjectError> {
        if !is_safe_slug(slug) {
            return Err(ProjectError::InvalidPage {
                path: slug.to_string(),
                reason: "slug enthält unerlaubte Zeichen".into(),
            });
        }
        if let Some(parent) = new_parent {
            if !is_safe_slug(parent) {
                return Err(ProjectError::InvalidPage {
                    path: parent.to_string(),
                    reason: "new_parent enthält unerlaubte Zeichen".into(),
                });
            }
            // Cycle-Verbot: new_parent darf nicht slug selbst oder ein Nachfahre sein.
            if parent == slug || parent.starts_with(&format!("{slug}/")) {
                return Err(ProjectError::InvalidPage {
                    path: parent.to_string(),
                    reason: "cycle: page kann nicht in ihren eigenen Nachfahren verschoben werden".into(),
                });
            }
            // Parent-Page muss existieren.
            if !self.page_path(parent).exists() {
                return Err(ProjectError::InvalidPage {
                    path: parent.to_string(),
                    reason: "new_parent-page existiert nicht".into(),
                });
            }
        }

        let current_parent = slug.rsplit_once('/').map(|(p, _)| p);
        let tail = slug.rsplit_once('/').map(|(_, t)| t).unwrap_or(slug);

        let parent_changed = current_parent != new_parent;
        let new_slug = match new_parent {
            Some(p) => format!("{p}/{tail}"),
            None => tail.to_string(),
        };

        if parent_changed {
            self.rename_page(slug, &new_slug)?;
        }

        if let Some(order) = new_order {
            let mut page = self.load_page(&new_slug)?;
            page.frontmatter.menu.order = Some(order);
            self.save_page_full(&new_slug, &page.frontmatter, &page.body_markdown)?;
        }

        Ok(new_slug)
    }

    /// Liefert die Slugs aller Kinder einer Page (Pages, deren Slug mit
    /// `<parent>/` beginnt). Eine Page mit nicht-existierendem Section-
    /// Verzeichnis hat keine Kinder.
    pub fn child_slugs(&self, parent_slug: &str) -> Result<Vec<String>, ProjectError> {
        let section = self.section_dir(parent_slug);
        if !section.is_dir() {
            return Ok(Vec::new());
        }
        let prefix = format!("{parent_slug}/");
        let mut out = Vec::new();
        for entry in walkdir::WalkDir::new(&section).min_depth(1) {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let utf8 = Utf8Path::from_path(path).ok_or(ProjectError::NonUtf8Path)?;
            let rel = utf8
                .strip_prefix(&self.pages_dir())
                .map_err(|_| ProjectError::NonUtf8Path)?;
            let slug = rel.as_str().trim_end_matches(".md").replace('\\', "/");
            if slug.starts_with(&prefix) {
                out.push(slug);
            }
        }
        out.sort();
        Ok(out)
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
        let path = self.page_path(slug);
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
        let path = self.page_path(slug);
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
    /// Benennt eine Page um — inklusive ihres Section-Verzeichnisses (Kinder),
    /// falls vorhanden, und der Backups. Schlägt fehl, wenn ein Konflikt mit
    /// dem Ziel-Slug oder dessen Section-Verzeichnis besteht.
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
        let old_path = self.page_path(old_slug);
        let new_path = self.page_path(new_slug);
        let old_section = self.section_dir(old_slug);
        let new_section = self.section_dir(new_slug);

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
        if new_section.exists() {
            return Err(ProjectError::InvalidPage {
                path: new_section.to_string(),
                reason: "ziel-section-verzeichnis existiert bereits".into(),
            });
        }

        // Parent-Dir des Ziels bei Bedarf anlegen (für `a` → `b/c`).
        if let Some(parent) = new_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::rename(&old_path, &new_path)?;

        // Kinder mitnehmen
        if old_section.is_dir() {
            std::fs::rename(&old_section, &new_section)?;
        }

        // Backups mitziehen (verschachtelt)
        let old_backup = self.root.join(".siteeditor/backups").join(old_slug);
        let new_backup = self.root.join(".siteeditor/backups").join(new_slug);
        if old_backup.exists() && !new_backup.exists() {
            if let Some(parent) = new_backup.parent() {
                std::fs::create_dir_all(parent)?;
            }
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

    // --- Phase 10: Deploy-Profile ---------------------------------------

    /// Service-Name fürs OS-Keystore — eindeutig pro (Projekt-Root, Profil).
    /// Format: `siteeditor.deploy.<project_hash>.<profile_name>`.
    /// Der Hash macht Projekte mit gleichem Profilnamen ungefährlich
    /// (z.B. zwei lokale Kopien desselben Repos).
    pub fn keystore_service_for(&self, profile_name: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(self.root.as_str().as_bytes());
        let digest = h.finalize();
        let mut hex = String::with_capacity(16);
        for b in &digest[..8] {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
        }
        format!("siteeditor.deploy.{hex}.{profile_name}")
    }

    /// Alle gespeicherten Profile (ohne Credentials). Klont aus dem
    /// in-memory-Manifest.
    pub fn list_deploy_profiles(&self) -> Vec<DeployProfile> {
        self.manifest.deploy_profiles.clone()
    }

    pub fn get_deploy_profile(&self, name: &str) -> Option<DeployProfile> {
        self.manifest
            .deploy_profiles
            .iter()
            .find(|p| p.name == name)
            .cloned()
    }

    /// Legt ein neues Profil an oder ersetzt ein gleichnamiges. Validiert
    /// das Profil vor dem Schreiben. Persistiert `site.json`.
    pub fn upsert_deploy_profile(&mut self, profile: DeployProfile) -> Result<(), ProjectError> {
        profile.validate().map_err(|e| ProjectError::InvalidSiteJson(e.to_string()))?;
        let mut new_manifest = self.manifest.clone();
        if let Some(existing) = new_manifest
            .deploy_profiles
            .iter_mut()
            .find(|p| p.name == profile.name)
        {
            *existing = profile;
        } else {
            new_manifest.deploy_profiles.push(profile);
        }
        // sortiert speichern → stabile JSON-Reihenfolge
        new_manifest.deploy_profiles.sort_by(|a, b| a.name.cmp(&b.name));
        self.persist_manifest(new_manifest)
    }

    /// Entfernt ein Profil per Name. Kein Fehler, wenn das Profil nicht
    /// existiert — Caller braucht keine Vorab-Prüfung.
    pub fn delete_deploy_profile(&mut self, name: &str) -> Result<(), ProjectError> {
        let mut new_manifest = self.manifest.clone();
        let before = new_manifest.deploy_profiles.len();
        new_manifest.deploy_profiles.retain(|p| p.name != name);
        if new_manifest.deploy_profiles.len() == before {
            return Ok(());
        }
        self.persist_manifest(new_manifest)
    }

    fn persist_manifest(&mut self, new_manifest: SiteManifest) -> Result<(), ProjectError> {
        let serialized = serde_json::to_string_pretty(&new_manifest)
            .map_err(|e| ProjectError::InvalidSiteJson(e.to_string()))?;
        atomic_write(&self.root.join("site.json"), &serialized)?;
        self.manifest = new_manifest;
        Ok(())
    }

    /// Löscht eine Page. Backups verbleiben unter `.siteeditor/backups/<slug>/`
    /// zur Wiederherstellung.
    pub fn delete_page(&self, slug: &str) -> Result<(), ProjectError> {
        let path = self.page_path(slug);
        if !path.exists() {
            return Err(ProjectError::InvalidPage {
                path: path.to_string(),
                reason: "page existiert nicht".into(),
            });
        }
        // Existieren Kinder, blockieren wir das Löschen — sonst hingen die
        // Kinder nach dem Löschen unter einer URL, deren Parent-Section weg ist.
        let children = self.child_slugs(slug)?;
        if !children.is_empty() {
            return Err(ProjectError::InvalidPage {
                path: path.to_string(),
                reason: format!(
                    "page hat {} Kind-Page(s); zuerst diese verschieben oder löschen",
                    children.len()
                ),
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

/// Slug-Sicherheit. Slugs sind Pfade aus `/`-getrennten Segmenten, z.B.
/// `about` oder `about/team`. Jedes Segment muss:
/// - nicht leer sein
/// - kein `\` enthalten
/// - nicht `.` oder `..` sein
/// - nicht mit `.` beginnen (keine versteckten Dateien)
/// Der Gesamt-Slug darf zudem keinen führenden/trailing Slash haben.
/// Strenge Kebab-Case-Regeln prüft `theme_contract::is_valid_slug`.
fn is_safe_slug(slug: &str) -> bool {
    if slug.is_empty() || slug.starts_with('/') || slug.ends_with('/') {
        return false;
    }
    if slug.contains('\\') {
        return false;
    }
    for seg in slug.split('/') {
        if seg.is_empty() || seg == "." || seg == ".." || seg.starts_with('.') {
            return false;
        }
    }
    true
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
        for bad in ["../etc", "..", ".hidden", "", "/abs", "trail/", "a//b", "a/./b", "a/../b", "a/.hidden", "a\\b"] {
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

    // --- themes --------------------------------------------------------------

    fn write_theme(project: &Project, slug: &str, display_name: Option<&str>) {
        let dir = project.themes_dir().join(slug);
        std::fs::create_dir_all(&dir).unwrap();
        let body = match display_name {
            Some(n) => format!(r#"{{"spec_version":"0.2","name":"{slug}","display_name":"{n}"}}"#),
            None => format!(r#"{{"spec_version":"0.2","name":"{slug}"}}"#),
        };
        std::fs::write(dir.join("theme.json"), body).unwrap();
    }

    #[test]
    fn list_installed_themes_sortiert_und_nimmt_display_name() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        write_theme(&project, "bravo", Some("Bravo Theme"));
        write_theme(&project, "alpha", None);
        // ignoriert: kein theme.json
        std::fs::create_dir_all(project.themes_dir().join("kaputt")).unwrap();
        // ignoriert: versteckte Verzeichnisse
        write_theme(&project, ".hidden", Some("X"));

        let themes = project.list_installed_themes().unwrap();
        let slugs: Vec<_> = themes.iter().map(|t| t.slug.clone()).collect();
        assert_eq!(slugs, vec!["alpha", "bravo"]);
        assert_eq!(themes[0].display_name, "alpha"); // fallback
        assert_eq!(themes[1].display_name, "Bravo Theme");
    }

    #[test]
    fn list_installed_themes_leer_wenn_dir_fehlt() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        assert!(project.list_installed_themes().unwrap().is_empty());
    }

    #[test]
    fn set_active_theme_persistiert_und_aktualisiert_manifest() {
        let (_tmp, mut project) = make_project_with_page("---\ntitle: T\n---\n\n");
        write_theme(&project, "default", Some("Default"));
        write_theme(&project, "mocha", Some("Mocha"));

        project.set_active_theme("mocha").unwrap();
        assert_eq!(project.manifest.active_theme, "mocha");

        // beim erneuten Öffnen persistent
        let reopened = Project::open(&project.root).unwrap();
        assert_eq!(reopened.manifest.active_theme, "mocha");
    }

    #[test]
    fn set_active_theme_lehnt_unbekanntes_theme_ab() {
        let (_tmp, mut project) = make_project_with_page("---\ntitle: T\n---\n\n");
        let err = project.set_active_theme("ghost").unwrap_err();
        assert!(matches!(err, ProjectError::InvalidPage { .. }));
        assert_eq!(project.manifest.active_theme, "default"); // unverändert
    }

    #[test]
    fn read_und_write_theme_css_roundtrip() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        write_theme(&project, "default", Some("Default"));
        let css_path = project.themes_dir().join("default/styles/main.css");
        std::fs::create_dir_all(css_path.parent().unwrap()).unwrap();
        std::fs::write(&css_path, "body { color: red; }\n").unwrap();

        assert_eq!(project.read_theme_css("default").unwrap(), "body { color: red; }\n");
        project.write_theme_css("default", "body { color: blue; }").unwrap();
        // \n wird ergänzt
        assert_eq!(project.read_theme_css("default").unwrap(), "body { color: blue; }\n");
    }

    #[test]
    fn theme_css_lehnt_unbekanntes_theme_und_unsichere_slugs_ab() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        write_theme(&project, "default", Some("Default"));
        // theme ohne styles/main.css
        let err = project.read_theme_css("default").unwrap_err();
        assert!(matches!(err, ProjectError::InvalidPage { .. }));

        for bad in ["../etc", "..", ".hidden", "", "/abs", "trail/", "a//b", "a/./b", "a/../b", "a/.hidden", "a\\b"] {
            let err = project.read_theme_css(bad).unwrap_err();
            assert!(matches!(err, ProjectError::InvalidPage { .. }), "slug {bad} sollte abgelehnt werden");
            let err = project.write_theme_css(bad, "x").unwrap_err();
            assert!(matches!(err, ProjectError::InvalidPage { .. }), "slug {bad} sollte abgelehnt werden");
        }
    }

    #[test]
    fn set_active_theme_lehnt_unsichere_slugs_ab() {
        let (_tmp, mut project) = make_project_with_page("---\ntitle: T\n---\n\n");
        for bad in ["../etc", "..", ".hidden", "", "/abs", "trail/", "a//b", "a/./b", "a/../b", "a/.hidden", "a\\b"] {
            let err = project.set_active_theme(bad).unwrap_err();
            assert!(matches!(err, ProjectError::InvalidPage { .. }), "slug {bad} sollte abgelehnt werden");
        }
    }

    // --- hierarchische Slugs (Modell B: Filesystem-Hierarchie) --------------

    #[test]
    fn list_pages_findet_subpages_rekursiv_und_setzt_slug_auf_relpfad() {
        let (_tmp, project) = make_project_with_page("---\ntitle: Home\n---\n\n");
        std::fs::create_dir_all(project.pages_dir().join("about")).unwrap();
        std::fs::write(project.pages_dir().join("about.md"), "---\ntitle: A\n---\n").unwrap();
        std::fs::write(project.pages_dir().join("about/team.md"), "---\ntitle: T\n---\n").unwrap();
        std::fs::create_dir_all(project.pages_dir().join("about/team")).unwrap();
        std::fs::write(project.pages_dir().join("about/team/role.md"), "---\ntitle: R\n---\n").unwrap();

        let slugs: Vec<_> = project.list_pages().unwrap().into_iter().map(|p| p.slug).collect();
        assert_eq!(slugs, vec!["about", "about/team", "about/team/role", "index"]);
    }

    #[test]
    fn create_und_load_page_mit_pfad_slug() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about/team", &fm("Team"), "body\n").unwrap();
        assert!(project.pages_dir().join("about/team.md").exists());
        let loaded = project.load_page("about/team").unwrap();
        assert_eq!(loaded.slug, "about/team");
        assert_eq!(loaded.frontmatter.title, "Team");
    }

    #[test]
    fn child_slugs_liefert_alle_kinder_rekursiv() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        project.create_page("about/team", &fm("T"), "").unwrap();
        project.create_page("about/team/role", &fm("R"), "").unwrap();
        project.create_page("contact", &fm("C"), "").unwrap();

        let children = project.child_slugs("about").unwrap();
        assert_eq!(children, vec!["about/team", "about/team/role"]);
        assert!(project.child_slugs("contact").unwrap().is_empty());
    }

    #[test]
    fn delete_page_lehnt_ab_wenn_kinder_existieren() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        project.create_page("about/team", &fm("T"), "").unwrap();

        let err = project.delete_page("about").unwrap_err();
        match err {
            ProjectError::InvalidPage { reason, .. } => assert!(reason.contains("Kind-Page")),
            other => panic!("unerwarteter Fehler: {other:?}"),
        }
        // erst Kind weg, dann Parent
        project.delete_page("about/team").unwrap();
        project.delete_page("about").unwrap();
    }

    #[test]
    fn rename_page_zieht_kinder_und_backups_mit() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        project.create_page("about/team", &fm("T"), "").unwrap();
        // backup erzeugen
        project.save_page_full("about", &fm("A2"), "").unwrap();

        project.rename_page("about", "company").unwrap();

        assert!(!project.pages_dir().join("about.md").exists());
        assert!(!project.pages_dir().join("about").exists());
        assert!(project.pages_dir().join("company.md").exists());
        assert!(project.pages_dir().join("company/team.md").exists());
        assert!(!project.root.join(".siteeditor/backups/about").exists());
        assert!(project.root.join(".siteeditor/backups/company").exists());
    }

    #[test]
    fn rename_page_kann_in_subpath_verschieben() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("contact", &fm("C"), "").unwrap();

        project.rename_page("contact", "about/contact").unwrap();
        assert!(!project.pages_dir().join("contact.md").exists());
        assert!(project.pages_dir().join("about/contact.md").exists());
    }

    #[test]
    fn rename_page_lehnt_kollision_mit_existierendem_section_dir_ab() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        project.create_page("about/team", &fm("T"), "").unwrap(); // legt about/ als Section an
        project.create_page("company", &fm("C"), "").unwrap();

        let err = project.rename_page("company", "about").unwrap_err();
        assert!(matches!(err, ProjectError::InvalidPage { .. }));
    }

    #[test]
    fn set_favorite_persistiert_im_frontmatter() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        assert!(!project.load_page("about").unwrap().frontmatter.favorite);

        project.set_favorite("about", true).unwrap();
        assert!(project.load_page("about").unwrap().frontmatter.favorite);

        project.set_favorite("about", false).unwrap();
        assert!(!project.load_page("about").unwrap().frontmatter.favorite);
    }

    #[test]
    fn move_page_reparent_aendert_slug_und_zieht_kinder_mit() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        project.create_page("about/team", &fm("T"), "").unwrap();

        let new_slug = project.move_page("about/team", Some("index"), None).unwrap();
        assert_eq!(new_slug, "index/team");
        assert!(project.pages_dir().join("index/team.md").exists());
        assert!(!project.pages_dir().join("about/team.md").exists());
    }

    #[test]
    fn move_page_promote_zu_root() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        project.create_page("about/team", &fm("T"), "").unwrap();

        let new_slug = project.move_page("about/team", None, None).unwrap();
        assert_eq!(new_slug, "team");
        assert!(project.pages_dir().join("team.md").exists());
    }

    #[test]
    fn move_page_setzt_menu_order_ohne_reparent() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();

        let new_slug = project.move_page("about", None, Some(42)).unwrap();
        assert_eq!(new_slug, "about");
        let reloaded = project.load_page("about").unwrap();
        assert_eq!(reloaded.frontmatter.menu.order, Some(42));
    }

    #[test]
    fn move_page_lehnt_cycle_ab() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        project.create_page("about/team", &fm("T"), "").unwrap();

        // about → about/team (eigener Nachfahre)
        let err = project.move_page("about", Some("about/team"), None).unwrap_err();
        match err {
            ProjectError::InvalidPage { reason, .. } => assert!(reason.contains("cycle")),
            other => panic!("unerwarteter Fehler: {other:?}"),
        }
        // about → about (self)
        let err = project.move_page("about", Some("about"), None).unwrap_err();
        assert!(matches!(err, ProjectError::InvalidPage { .. }));
    }

    #[test]
    fn move_page_lehnt_nicht_existenten_parent_ab() {
        let (_tmp, project) = make_project_with_page("---\ntitle: H\n---\n\n");
        project.create_page("about", &fm("A"), "").unwrap();
        let err = project.move_page("about", Some("ghost"), None).unwrap_err();
        assert!(matches!(err, ProjectError::InvalidPage { .. }));
    }

    #[test]
    fn is_safe_slug_accepts_kebab_paths_und_blockt_traversal() {
        assert!(is_safe_slug("about"));
        assert!(is_safe_slug("about/team"));
        assert!(is_safe_slug("a/b/c/d"));
        // Blockierte Slugs
        for bad in ["", "/", "/abs", "trail/", "..", "../etc", "a/../b", "a/./b", "a//b", ".hidden", "a/.hidden", "a\\b"] {
            assert!(!is_safe_slug(bad), "slug {bad:?} sollte abgelehnt werden");
        }
    }

    // --- Phase 10: deploy-Profile -------------------------------------------

    use deploy_contract::{AuthMethod, Protocol};

    fn sftp_profile(name: &str) -> DeployProfile {
        DeployProfile {
            name: name.into(),
            protocol: Protocol::Sftp,
            host: "example.com".into(),
            port: 22,
            auth: AuthMethod::Password { user: "deploy".into() },
            remote_path: "/var/www/site".into(),
            branch: None,
            prefer_diff: true,
        }
    }

    #[test]
    fn neues_projekt_hat_keine_deploy_profile() {
        let (_tmp, project) = make_project_with_page("---\ntitle: T\n---\n\n");
        assert!(project.list_deploy_profiles().is_empty());
    }

    #[test]
    fn upsert_legt_profil_an_und_persistiert_ohne_credentials() {
        let (_tmp, mut project) = make_project_with_page("---\ntitle: T\n---\n\n");
        project.upsert_deploy_profile(sftp_profile("prod")).unwrap();
        assert_eq!(project.list_deploy_profiles().len(), 1);

        // beim Reload weiterhin da
        let reopened = Project::open(&project.root).unwrap();
        assert_eq!(reopened.manifest.deploy_profiles.len(), 1);
        assert_eq!(reopened.manifest.deploy_profiles[0].name, "prod");

        // site.json enthält KEIN Passwort-Feld
        let raw = std::fs::read_to_string(project.root.join("site.json")).unwrap();
        assert!(!raw.to_lowercase().contains("password\":"), "site.json darf kein Passwort enthalten: {raw}");
        assert!(!raw.to_lowercase().contains("secret"), "site.json darf kein Secret enthalten: {raw}");
    }

    #[test]
    fn upsert_ersetzt_gleichnamiges_profil() {
        let (_tmp, mut project) = make_project_with_page("---\ntitle: T\n---\n\n");
        project.upsert_deploy_profile(sftp_profile("prod")).unwrap();
        let mut updated = sftp_profile("prod");
        updated.host = "neuer-host.example".into();
        project.upsert_deploy_profile(updated).unwrap();
        let profiles = project.list_deploy_profiles();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].host, "neuer-host.example");
    }

    #[test]
    fn upsert_sortiert_nach_name() {
        let (_tmp, mut project) = make_project_with_page("---\ntitle: T\n---\n\n");
        project.upsert_deploy_profile(sftp_profile("staging")).unwrap();
        project.upsert_deploy_profile(sftp_profile("prod")).unwrap();
        project.upsert_deploy_profile(sftp_profile("dev")).unwrap();
        let names: Vec<_> = project
            .list_deploy_profiles()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["dev", "prod", "staging"]);
    }

    #[test]
    fn upsert_lehnt_invalide_profile_ab() {
        let (_tmp, mut project) = make_project_with_page("---\ntitle: T\n---\n\n");
        let mut bad = sftp_profile("ok");
        bad.host = "".into();
        let err = project.upsert_deploy_profile(bad).unwrap_err();
        assert!(matches!(err, ProjectError::InvalidSiteJson(_)));
        // Original-Manifest unverändert
        assert!(project.list_deploy_profiles().is_empty());
    }

    #[test]
    fn delete_entfernt_und_ist_idempotent() {
        let (_tmp, mut project) = make_project_with_page("---\ntitle: T\n---\n\n");
        project.upsert_deploy_profile(sftp_profile("prod")).unwrap();
        project.delete_deploy_profile("prod").unwrap();
        assert!(project.list_deploy_profiles().is_empty());
        // zweimal löschen ist ok
        project.delete_deploy_profile("prod").unwrap();
    }

    #[test]
    fn keystore_service_ist_projekt_spezifisch_und_stabil() {
        let (_tmp_a, project_a) = make_project_with_page("---\ntitle: T\n---\n\n");
        let (_tmp_b, project_b) = make_project_with_page("---\ntitle: T\n---\n\n");

        let a1 = project_a.keystore_service_for("prod");
        let a2 = project_a.keystore_service_for("prod");
        let b1 = project_b.keystore_service_for("prod");
        assert_eq!(a1, a2, "Service-Name muss stabil sein");
        assert_ne!(a1, b1, "verschiedene Projekt-Roots → verschiedene Service-Namen");
        assert!(a1.starts_with("siteeditor.deploy."));
        assert!(a1.ends_with(".prod"));
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
