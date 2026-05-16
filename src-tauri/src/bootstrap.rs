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

    // pages
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
  - type: cta
    text: "Mehr erfahren"
    href: "/about/"
    style: primary
---

Das hier ist der **Prosa-Bereich**, den der Editor später als WYSIWYG zeigt.

Eine Liste:

- Punkt eins
- Punkt zwei
- Punkt drei
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
  - type: quote
    text: "Das Beste, was du tun kannst, ist anfangen."
    author: "Unbekannt"
---

Hier kommt eine kurze Beschreibung dessen, was wir tun.

## Unsere Werte

Schlicht. Schnell. Verständlich.
"#;
    write(target.join("pages").join("about.md"), about_md)?;

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

fn write_default_theme(dir: &Utf8Path) -> Result<()> {
    write(dir.join("theme.json"), include_str!("../../themes/default/theme.json"))?;
    write(dir.join("styles/main.css"), include_str!("../../themes/default/styles/main.css"))?;
    write(dir.join("templates/page.html"), include_str!("../../themes/default/templates/page.html"))?;
    write(dir.join("templates/index.html"), include_str!("../../themes/default/templates/index.html"))?;
    write(dir.join("templates/404.html"), include_str!("../../themes/default/templates/404.html"))?;
    write(dir.join("templates/partials/head.html"), include_str!("../../themes/default/templates/partials/head.html"))?;
    write(dir.join("templates/partials/menu.html"), include_str!("../../themes/default/templates/partials/menu.html"))?;
    Ok(())
}
