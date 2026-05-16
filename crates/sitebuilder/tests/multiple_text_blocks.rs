//! Im neuen Block-Modell (Phase 06.1) trägt jeder `text`-Block sein eigenes
//! Markdown im `content`-Feld. Mehrere Text-Blocks pro Page sind erlaubt und
//! müssen unabhängig gerendert werden.

use camino::Utf8PathBuf;
use projectfs::Project;

fn write(path: &Utf8PathBuf, content: &str) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

fn write_default_theme(theme: &Utf8PathBuf) {
    write(&theme.join("theme.json"), include_str!("../../../themes/default/theme.json"));
    write(&theme.join("templates/page.html"), include_str!("../../../themes/default/templates/page.html"));
    write(&theme.join("templates/index.html"), include_str!("../../../themes/default/templates/index.html"));
    write(&theme.join("templates/404.html"), include_str!("../../../themes/default/templates/404.html"));
    write(&theme.join("templates/partials/head.html"), include_str!("../../../themes/default/templates/partials/head.html"));
    write(&theme.join("templates/partials/menu.html"), include_str!("../../../themes/default/templates/partials/menu.html"));
    write(&theme.join("templates/partials/_menu_macros.html"), include_str!("../../../themes/default/templates/partials/_menu_macros.html"));
    write(&theme.join("styles/main.css"), include_str!("../../../themes/default/styles/main.css"));
}

#[test]
fn multiple_text_blocks_render_independently() {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

    write(
        &root.join("site.json"),
        r#"{"schema_version":"0.2","title":"T","base_url":"https://x","active_theme":"default"}"#,
    );
    write(
        &root.join("pages/index.md"),
        "---\n\
         title: Home\n\
         template: index\n\
         visible: true\n\
         blocks:\n  \
           - type: text\n    \
             content: \"Erster **Absatz** mit fettem Wort.\"\n  \
           - type: text\n    \
             content: \"Zweiter Absatz mit eigener *Kursivierung*.\"\n\
         ---\n",
    );
    write_default_theme(&root.join("themes/default"));

    let project = Project::open(&root).expect("open project");
    sitebuilder::build_site(&project).expect("build sollte erfolgreich sein");

    let html = std::fs::read_to_string(root.join(".siteeditor/build/index.html")).unwrap();
    assert!(html.contains("<strong>Absatz</strong>"), "erster Text-Block fehlt: {html}");
    assert!(html.contains("<em>Kursivierung</em>"), "zweiter Text-Block fehlt: {html}");
}

#[test]
fn text_block_inside_columns_renders_own_content() {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

    write(
        &root.join("site.json"),
        r#"{"schema_version":"0.2","title":"T","base_url":"https://x","active_theme":"default"}"#,
    );
    write(
        &root.join("pages/index.md"),
        "---\n\
         title: Home\n\
         template: index\n\
         visible: true\n\
         blocks:\n  \
           - type: columns\n    \
             columns:\n      \
               - - type: text\n          \
                   content: \"Links: erster Sub-Block.\"\n      \
               - - type: text\n          \
                   content: \"Rechts: zweiter Sub-Block.\"\n\
         ---\n",
    );
    write_default_theme(&root.join("themes/default"));

    let project = Project::open(&root).expect("open project");
    sitebuilder::build_site(&project).expect("build sollte erfolgreich sein");

    let html = std::fs::read_to_string(root.join(".siteeditor/build/index.html")).unwrap();
    assert!(html.contains("Links: erster Sub-Block"), "linker Sub-Block fehlt: {html}");
    assert!(html.contains("Rechts: zweiter Sub-Block"), "rechter Sub-Block fehlt: {html}");
}
