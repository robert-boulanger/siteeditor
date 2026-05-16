//! End-to-end smoke: bootstrap a project via raw file writes, build it.

use camino::Utf8PathBuf;
use projectfs::Project;

fn write(path: &Utf8PathBuf, content: &str) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

#[test]
fn bootstrap_and_build() {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

    write(
        &root.join("site.json"),
        r#"{"schema_version":"0.2","title":"T","base_url":"https://x","active_theme":"default"}"#,
    );
    write(
        &root.join("pages/index.md"),
        "---\ntitle: Home\ntemplate: index\nvisible: true\nmenu:\n  show: true\n  order: 1\nblocks:\n  - type: hero\n    headline: Hi\n  - type: text\n    content: \"# Hello\"\n---\n",
    );

    let theme = root.join("themes/default");
    write(&theme.join("theme.json"), include_str!("../../../themes/default/theme.json"));
    write(&theme.join("templates/page.html"), include_str!("../../../themes/default/templates/page.html"));
    write(&theme.join("templates/index.html"), include_str!("../../../themes/default/templates/index.html"));
    write(&theme.join("templates/404.html"), include_str!("../../../themes/default/templates/404.html"));
    write(&theme.join("templates/partials/head.html"), include_str!("../../../themes/default/templates/partials/head.html"));
    write(&theme.join("templates/partials/menu.html"), include_str!("../../../themes/default/templates/partials/menu.html"));
    write(&theme.join("templates/partials/_menu_macros.html"), include_str!("../../../themes/default/templates/partials/_menu_macros.html"));
    write(&theme.join("styles/main.css"), include_str!("../../../themes/default/styles/main.css"));

    let project = Project::open(&root).expect("open project");
    let report = sitebuilder::build_site(&project).expect("build");
    assert_eq!(report.pages_rendered, 1);

    let html = std::fs::read_to_string(root.join(".siteeditor/build/index.html")).unwrap();
    assert!(html.contains("Hi"), "hero headline missing");
    assert!(html.contains("<h1>Hello</h1>"), "page body not rendered");
    assert!(html.contains("/styles/main.css"));
}
