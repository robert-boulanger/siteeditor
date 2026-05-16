//! Public-API-Vertrag des `default`-Themes: stabile BEM-Klassen pro Blocktyp.
//!
//! Diese Tests sind der Grund-Vertrag für Theme-Autoren (siehe
//! `THEME_AUTHORING.md`). Wenn hier eine Klasse fällt, ist das ein
//! Breaking Change für alle abgeleiteten Themes — bewusst entscheiden.
//!
//! Phase 09 (Session 6): Konvention etabliert.

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

fn build_with(page_md: &str) -> (String, String) {
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();

    write(
        &root.join("site.json"),
        r#"{"schema_version":"0.2","title":"T","base_url":"https://x","active_theme":"default"}"#,
    );
    write(&root.join("pages/index.md"), page_md);
    write_default_theme(&root.join("themes/default"));

    let project = Project::open(&root).expect("open project");
    sitebuilder::build_site(&project).expect("build");

    let index = std::fs::read_to_string(root.join(".siteeditor/build/index.html")).unwrap();
    let four_oh_four =
        std::fs::read_to_string(root.join(".siteeditor/build/404.html")).unwrap();
    // tempdir lebt bis Ende der Funktion — beide Strings sind besitzt.
    let _ = tmp;
    (index, four_oh_four)
}

fn assert_has(html: &str, needle: &str) {
    assert!(html.contains(needle), "fehlt im HTML: `{needle}`\n--- HTML ---\n{html}");
}

#[test]
fn block_wrapper_klasse_ist_pflicht_fuer_jeden_blocktyp() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks:\n  \
        - type: hero\n    headline: H\n  \
        - type: text\n    content: x\n  \
        - type: image\n    image: /img.png\n  \
        - type: gallery\n    images: [{src: /a.png}]\n  \
        - type: video\n    source: /v.mp4\n  \
        - type: cta\n    text: ok\n    href: /\n  \
        - type: quote\n    text: q\n---\n";
    let (html, _) = build_with(md);
    for cls in [
        "class=\"block hero",
        "class=\"block prose",
        "class=\"block image",
        "class=\"block gallery",
        "class=\"block video",
        "class=\"block cta\"",
        "class=\"block quote\"",
    ] {
        assert_has(&html, cls);
    }
}

#[test]
fn hero_bem_elemente_und_align_modifier() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks:\n  \
        - type: hero\n    headline: Hi\n    sub: Sub\n    align: left\n    image: /a.png\n    image_caption: cap\n---\n";
    let (html, _) = build_with(md);
    assert_has(&html, "hero--align-left");
    assert_has(&html, "class=\"hero__figure\"");
    assert_has(&html, "class=\"hero__image\"");
    assert_has(&html, "class=\"hero__caption\"");
    assert_has(&html, "class=\"hero__headline\"");
    assert_has(&html, "class=\"hero__sub\"");
}

#[test]
fn image_bem_elemente_und_width_modifier() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks:\n  \
        - type: image\n    image: /x.png\n    caption: c\n    width: full\n---\n";
    let (html, _) = build_with(md);
    assert_has(&html, "image--full");
    assert_has(&html, "class=\"image__img\"");
    assert_has(&html, "class=\"image__caption\"");
}

#[test]
fn gallery_bem_elemente() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks:\n  \
        - type: gallery\n    images:\n      - {src: /a.png, caption: A}\n      - {src: /b.png}\n---\n";
    let (html, _) = build_with(md);
    assert_has(&html, "class=\"gallery__item\"");
    assert_has(&html, "class=\"gallery__image\"");
    assert_has(&html, "class=\"gallery__caption\"");
}

#[test]
fn video_bem_elemente() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks:\n  \
        - type: video\n    source: /v.mp4\n    caption: cap\n---\n";
    let (html, _) = build_with(md);
    assert_has(&html, "class=\"video__player\"");
    assert_has(&html, "class=\"video__caption\"");
}

#[test]
fn cta_ist_div_nicht_p_und_hat_btn_modifier() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks:\n  \
        - type: cta\n    text: Los\n    href: /\n    style: secondary\n---\n";
    let (html, _) = build_with(md);
    assert_has(&html, "<div class=\"block cta\">");
    assert!(
        !html.contains("<p class=\"block cta\""),
        "CTA darf kein <p> mehr sein"
    );
    assert_has(&html, "cta__btn--secondary");
}

#[test]
fn quote_bem_elemente() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks:\n  \
        - type: quote\n    text: hello\n    author: X\n    source: Y\n---\n";
    let (html, _) = build_with(md);
    assert_has(&html, "class=\"quote__text\"");
    assert_has(&html, "class=\"quote__cite\"");
}

#[test]
fn columns_innere_items_tragen_kontext_klassen() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks:\n  \
        - type: columns\n    columns:\n      \
            - - type: text\n          content: txt\n      \
            - - type: cta\n          text: ok\n          href: /\n---\n";
    let (html, _) = build_with(md);
    assert_has(&html, "class=\"columns__col\"");
    assert_has(&html, "columns__item columns__item--text");
    assert_has(&html, "columns__item columns__item--cta");
}

#[test]
fn menu_rendert_hierarchisch_mit_nested_nav_list() {
    // Zwei Pages: parent + child. Beide im Menü sichtbar.
    let tmp = tempfile::tempdir().unwrap();
    let root = Utf8PathBuf::from_path_buf(tmp.path().to_path_buf()).unwrap();
    write(
        &root.join("site.json"),
        r#"{"schema_version":"0.2","title":"T","base_url":"https://x","active_theme":"default"}"#,
    );
    write(
        &root.join("pages/index.md"),
        "---\ntitle: Home\ntemplate: index\nvisible: true\nmenu:\n  show: true\n  order: 1\nblocks: []\n---\n",
    );
    write(
        &root.join("pages/about.md"),
        "---\ntitle: About\ntemplate: page\nvisible: true\nmenu:\n  show: true\n  order: 2\nblocks: []\n---\n",
    );
    write(
        &root.join("pages/about/team.md"),
        "---\ntitle: Team\ntemplate: page\nvisible: true\nmenu:\n  show: true\n  order: 1\nblocks: []\n---\n",
    );
    write_default_theme(&root.join("themes/default"));

    let project = Project::open(&root).expect("open");
    sitebuilder::build_site(&project).expect("build");

    let html = std::fs::read_to_string(root.join(".siteeditor/build/index.html")).unwrap();
    assert_has(&html, "nav-list--depth-0");
    assert_has(&html, "nav-list--depth-1");
    assert_has(&html, "has-children");
    assert_has(&html, "href=\"/about/team/\"");
    // Hamburger-Trigger ist im Markup
    assert_has(&html, "class=\"nav-toggle\"");
    assert_has(&html, "class=\"nav-burger\"");
}

#[test]
fn head_verlinkt_vars_css_vor_main_css() {
    // Regression: `_vars.css` (Theme-CSS-Variablen) wurde erzeugt, aber nie
    // ins HTML eingebunden. Folge: `var(--font-body)` undefined → Browser
    // fällt auf Serif zurück.
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks: []\n---\n";
    let (html, four) = build_with(md);
    for page in [&html, &four] {
        let vars_at = page.find("/styles/_vars.css").expect("_vars.css link fehlt");
        let main_at = page.find("/styles/main.css").expect("main.css link fehlt");
        assert!(vars_at < main_at, "_vars.css muss vor main.css verlinkt sein");
    }
}

#[test]
fn vierhundertvier_nutzt_head_partial_und_page_error_modifier() {
    let md = "---\ntitle: T\ntemplate: index\nvisible: true\nblocks: []\n---\n";
    let (_, four) = build_with(md);
    assert_has(&four, "/styles/main.css");
    assert_has(&four, "class=\"page page--error\"");
    assert_has(&four, "class=\"site-header\"");
    assert_has(&four, "<title>404 — T</title>");
}
