//! Sicherstellt, dass der Build mit klarem Fehler abbricht, wenn eine Page
//! mehr als einen `text`-Block deklariert.

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
    write(&theme.join("styles/main.css"), include_str!("../../../themes/default/styles/main.css"));
}

#[test]
fn build_aborts_on_multiple_text_blocks() {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

    write(
        &root.join("site.json"),
        r#"{"schema_version":"0.2","title":"T","base_url":"https://x","active_theme":"default"}"#,
    );
    write(
        &root.join("pages/index.md"),
        "---\ntitle: Home\ntemplate: index\nvisible: true\nblocks:\n  - type: text\n  - type: text\n---\n# Hello\n",
    );
    write_default_theme(&root.join("themes/default"));

    let project = Project::open(&root).expect("open project");
    let err = sitebuilder::build_site(&project).expect_err("build sollte fehlschlagen");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("MULTIPLE_TEXT_BLOCKS"),
        "Fehlertext enthält MULTIPLE_TEXT_BLOCKS nicht: {msg}"
    );
}
