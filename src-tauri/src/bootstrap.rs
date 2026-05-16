//! Legt ein lauffähiges Beispiel-Projekt im gewählten Verzeichnis an.

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

pub fn bootstrap_example_project(target: &Utf8Path) -> Result<()> {
    if !target.exists() {
        std::fs::create_dir_all(target)?;
    }
    if std::fs::read_dir(target)?.next().is_some() {
        return Err(anyhow!(
            "Zielverzeichnis nicht leer: {target}. Bitte leeren Ordner wählen."
        ));
    }

    // site.json
    let site_json = r#"{
  "schema_version": "0.2",
  "title": "Meine Beispielseite",
  "description": "Smoke-Test-Projekt für siteeditor.",
  "base_url": "https://example.com",
  "active_theme": "default",
  "language": "de",
  "menu_order": ["index", "about"],
  "css_var_overrides": {}
}
"#;
    write(target.join("site.json"), site_json)?;

    // pages — neues Modell: jeder text-Block trägt sein eigenes Markdown im
    // `content`-Feld. Es gibt keinen globalen Page-Body mehr.
    let index_md = r#"---
title: Willkommen
template: index
visible: true
menu:
  show: true
  order: 1
blocks:
  - type: hero
    headline: "Willkommen auf meiner Seite"
    sub: "Ein erster Smoke-Test mit siteeditor"
    align: center
  - type: text
    content: |
      Das hier ist der **Prosa-Bereich**, den der Editor als WYSIWYG zeigt.

      Eine Liste:

      - Punkt eins
      - Punkt zwei
      - Punkt drei
  - type: cta
    text: "Mehr erfahren"
    href: "/about/"
    style: primary
  - type: text
    content: |
      Und hier ein **zweiter** Text-Block — jeder Text-Block ist unabhängig
      editierbar, mit eigenem TipTap-Editor.
---
"#;
    write(target.join("pages").join("index.md"), index_md)?;

    let about_md = r#"---
title: Über uns
template: page
visible: true
menu:
  show: true
  order: 2
blocks:
  - type: hero
    headline: "Über uns"
  - type: text
    content: |
      Hier kommt eine kurze Beschreibung dessen, was wir tun.

      ## Unsere Werte

      Schlicht. Schnell. Verständlich.
  - type: quote
    text: "Das Beste, was du tun kannst, ist anfangen."
    author: "Unbekannt"
---
"#;
    write(target.join("pages").join("about.md"), about_md)?;

    // Subpage als Demo für hierarchische Pages (Modell B: Filesystem-Hierarchie).
    let team_md = r#"---
title: Team
template: page
visible: true
menu:
  show: true
  order: 1
blocks:
  - type: hero
    headline: "Unser Team"
  - type: text
    content: |
      Hier kommt die Vorstellung der Köpfe hinter dem Projekt.
---
"#;
    write(target.join("pages").join("about").join("team.md"), team_md)?;

    // themes/default -> copy from app bundle? Easier: write inline minimal default.
    write_default_theme(&target.join("themes").join("default"))?;

    // assets dir
    std::fs::create_dir_all(target.join("assets"))?;

    Ok(())
}

fn write(path: Utf8PathBuf, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {parent}"))?;
    }
    std::fs::write(&path, content).with_context(|| format!("write {path}"))?;
    Ok(())
}

fn write_bytes(path: Utf8PathBuf, content: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("mkdir {parent}"))?;
    }
    std::fs::write(&path, content).with_context(|| format!("write {path}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8Path;

    fn utf8(tmp: &tempfile::TempDir) -> Utf8PathBuf {
        Utf8Path::from_path(tmp.path()).unwrap().to_path_buf()
    }

    #[test]
    fn bootstraps_into_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let target = utf8(&tmp);
        bootstrap_example_project(&target).expect("bootstrap empty dir");

        assert!(target.join("site.json").exists());
        assert!(target.join("pages/index.md").exists());
        assert!(target.join("pages/about.md").exists());
        assert!(target.join("pages/about/team.md").exists());
        assert!(target.join("themes/default/theme.json").exists());
        assert!(target.join("themes/default/templates/index.html").exists());
        assert!(target.join("themes/default/assets/fonts/Inter-Regular.woff2").exists());
        assert!(target.join("themes/default/assets/fonts/Inter-SemiBold.woff2").exists());
        assert!(target.join("themes/default/assets/fonts/Inter-Bold.woff2").exists());
        assert!(target.join("assets").is_dir());
    }

    #[test]
    fn rejects_non_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let target = utf8(&tmp);
        std::fs::write(target.join("preexisting.txt"), "hi").unwrap();

        let err = bootstrap_example_project(&target).expect_err("nicht-leeres Verzeichnis ablehnen");
        let msg = format!("{err:#}");
        assert!(msg.contains("nicht leer"), "Fehlertext: {msg}");
    }

    #[test]
    fn creates_target_dir_if_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let target = utf8(&tmp).join("new-project");
        assert!(!target.exists());
        bootstrap_example_project(&target).expect("create + bootstrap");
        assert!(target.join("site.json").exists());
    }
}

fn write_default_theme(dir: &Utf8Path) -> Result<()> {
    write(dir.join("theme.json"), include_str!("../../themes/default/theme.json"))?;
    write(dir.join("styles/main.css"), include_str!("../../themes/default/styles/main.css"))?;
    write(dir.join("templates/page.html"), include_str!("../../themes/default/templates/page.html"))?;
    write(dir.join("templates/index.html"), include_str!("../../themes/default/templates/index.html"))?;
    write(dir.join("templates/404.html"), include_str!("../../themes/default/templates/404.html"))?;
    write(dir.join("templates/partials/head.html"), include_str!("../../themes/default/templates/partials/head.html"))?;
    write(dir.join("templates/partials/menu.html"), include_str!("../../themes/default/templates/partials/menu.html"))?;
    write(dir.join("templates/partials/_menu_macros.html"), include_str!("../../themes/default/templates/partials/_menu_macros.html"))?;
    // Inter-Fonts (OFL) lokal mitgeben — kein CDN.
    write_bytes(
        dir.join("assets/fonts/Inter-Regular.woff2"),
        include_bytes!("../../themes/default/assets/fonts/Inter-Regular.woff2"),
    )?;
    write_bytes(
        dir.join("assets/fonts/Inter-SemiBold.woff2"),
        include_bytes!("../../themes/default/assets/fonts/Inter-SemiBold.woff2"),
    )?;
    write_bytes(
        dir.join("assets/fonts/Inter-Bold.woff2"),
        include_bytes!("../../themes/default/assets/fonts/Inter-Bold.woff2"),
    )?;
    Ok(())
}
