//! SiteBuilder: rendert ein Site-Projekt + aktives Theme nach `<root>/.siteeditor/build/`.

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use projectfs::{PageDoc, Project};
use serde::Serialize;
use std::collections::BTreeMap;
use tera::{Context as TeraContext, Tera};

pub use projectfs;
pub use theme_contract;

#[derive(Debug, Serialize, Clone)]
pub struct BuildReport {
    pub pages_rendered: usize,
    pub output_dir: String,
    pub warnings: Vec<String>,
}

pub fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    opts.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(md, opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

#[derive(Debug, Serialize, Clone)]
struct MenuItem {
    title: String,
    slug: String,
    url: String,
    order: i32,
    #[serde(default)]
    children: Vec<MenuItem>,
}

#[derive(Debug, Serialize, Clone)]
struct SiteCtx<'a> {
    title: &'a str,
    description: Option<&'a str>,
    base_url: &'a str,
    language: String,
    menu: Vec<MenuItem>,
    theme: ThemeCtx<'a>,
}

#[derive(Debug, Serialize, Clone)]
struct ThemeCtx<'a> {
    name: &'a str,
    css_vars: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Clone)]
struct PageCtx {
    title: String,
    slug: String,
    url: String,
    template: String,
    meta: BTreeMap<String, String>,
    blocks: Vec<serde_json::Value>,
}

pub fn build_site(project: &Project) -> Result<BuildReport> {
    let theme_dir = project.active_theme_dir();
    let report = theme_contract::validate_theme(&theme_dir);
    if !report.ok {
        let msg = report
            .errors
            .iter()
            .map(|e| format!("[{}] {}", e.code, e.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(anyhow!("Theme-Validierung fehlgeschlagen: {msg}"));
    }

    let mut tera = Tera::default();
    let templates_dir = theme_dir.join("templates");
    add_templates(&mut tera, &templates_dir, "")?;

    let theme_manifest = load_theme_manifest(&theme_dir)?;
    let mut css_vars = theme_manifest.css_variables.clone();
    for (k, v) in &project.manifest.css_var_overrides {
        css_vars.insert(k.clone(), v.clone());
    }

    let pages = project.list_pages().context("list pages")?;

    let menu_items = build_menu(&pages, &project.manifest.menu_order);

    let site_ctx = SiteCtx {
        title: &project.manifest.title,
        description: project.manifest.description.as_deref(),
        base_url: &project.manifest.base_url,
        language: project.manifest.language.clone().unwrap_or_else(|| "en".into()),
        menu: menu_items,
        theme: ThemeCtx {
            name: &theme_manifest.name,
            css_vars: css_vars.clone(),
        },
    };

    let out_dir = project.build_dir();
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir).ok();
    }
    std::fs::create_dir_all(&out_dir).context("create build dir")?;

    for page in &pages {
        render_page(&tera, &site_ctx, page, &out_dir, &project.manifest)?;
    }
    // 404
    render_404(&tera, &site_ctx, &out_dir)?;

    // Theme-CSS + theme assets
    copy_dir(&theme_dir.join("styles"), &out_dir.join("styles")).ok();
    copy_dir(&theme_dir.join("assets"), &out_dir.join("theme-assets")).ok();
    // Project assets
    copy_dir(&project.assets_dir(), &out_dir.join("assets")).ok();

    // Inject css var overrides via styles/_vars.css link addendum
    write_css_vars(&out_dir, &css_vars).ok();

    Ok(BuildReport {
        pages_rendered: pages.len(),
        output_dir: out_dir.to_string(),
        warnings: Vec::new(),
    })
}

fn add_templates(tera: &mut Tera, dir: &Utf8PathBuf, prefix: &str) -> Result<()> {
    // Bulk-Variante: Tera löst Imports/Extends erst, wenn alle Templates
    // registriert sind. So spielt die Walk-Reihenfolge keine Rolle.
    let mut buffered: Vec<(String, String)> = Vec::new();
    for entry in walkdir::WalkDir::new(dir).min_depth(1) {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("html") {
            continue;
        }
        let rel = path.strip_prefix(dir).unwrap();
        let name = if prefix.is_empty() {
            rel.to_string_lossy().replace('\\', "/")
        } else {
            format!("{prefix}/{}", rel.to_string_lossy().replace('\\', "/"))
        };
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read template {}", path.display()))?;
        buffered.push((name, raw));
    }
    let refs: Vec<(&str, &str)> = buffered.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect();
    tera.add_raw_templates(refs)
        .context("add theme templates")?;
    Ok(())
}

fn load_theme_manifest(theme_dir: &Utf8PathBuf) -> Result<theme_contract::ThemeManifest> {
    let raw = std::fs::read_to_string(theme_dir.join("theme.json"))?;
    Ok(serde_json::from_str(&raw)?)
}

/// Baut einen Menü-Baum aus den (flach gelisteten) Pages. Eltern-Beziehung
/// wird aus dem Slug-Pfad abgeleitet: `about/team` ist Kind von `about`.
///
/// `site.menu_order` ordnet ausschließlich die **Top-Level-Items**. Kinder
/// werden pro Parent nach `menu.order` (dann nach Titel) sortiert.
///
/// Wenn die Parent-Page fehlt oder im Menü ausgeblendet ist, hängt der Knoten
/// trotzdem an seinem nächsten *vorhandenen* Vorfahren im Menü; existiert
/// keiner, wird er auf Root-Ebene gehängt (kein Eintrag wird unsichtbar).
fn build_menu(pages: &[PageDoc], order: &[String]) -> Vec<MenuItem> {
    let visible: Vec<&PageDoc> = pages
        .iter()
        .filter(|p| p.frontmatter.visible && p.frontmatter.menu.show)
        .collect();
    let visible_slugs: std::collections::HashSet<&str> =
        visible.iter().map(|p| p.slug.as_str()).collect();

    // Map slug -> MenuItem (zunächst ohne Kinder)
    let mut by_slug: BTreeMap<String, MenuItem> = visible
        .iter()
        .map(|p| {
            (
                p.slug.clone(),
                MenuItem {
                    title: p.frontmatter.title.clone(),
                    slug: p.slug.clone(),
                    url: page_url(&p.slug),
                    order: p.frontmatter.menu.order.unwrap_or(1000),
                    children: Vec::new(),
                },
            )
        })
        .collect();

    // Für jeden Slug: nächsten vorhandenen Vorfahren bestimmen.
    let mut parent_of: BTreeMap<String, Option<String>> = BTreeMap::new();
    for slug in by_slug.keys().cloned().collect::<Vec<_>>() {
        parent_of.insert(slug.clone(), nearest_visible_ancestor(&slug, &visible_slugs));
    }

    // Bottom-up zusammenstecken: Knoten ohne Kinder werden zuerst in ihren
    // Parent gehängt. BTreeMap-Iteration ist alphabetisch; wir gehen daher
    // absteigend nach Tiefe, damit Kinder ihre Kinder bereits drin haben.
    let mut slugs_by_depth: Vec<String> = by_slug.keys().cloned().collect();
    slugs_by_depth.sort_by_key(|s| std::cmp::Reverse(s.matches('/').count()));

    for slug in slugs_by_depth {
        if let Some(Some(parent_slug)) = parent_of.get(&slug).cloned() {
            // Knoten aus der Map entfernen und beim Parent einhängen.
            if let Some(node) = by_slug.remove(&slug) {
                if let Some(parent) = by_slug.get_mut(&parent_slug) {
                    parent.children.push(node);
                } else {
                    // Parent wurde bereits verschoben — zurücklegen als Root-Fallback.
                    by_slug.insert(slug, node);
                }
            }
        }
    }

    // Was jetzt noch in by_slug liegt, sind Root-Items.
    let mut roots: Vec<MenuItem> = by_slug.into_values().collect();

    // Kinder pro Knoten rekursiv sortieren (nach order, dann title).
    fn sort_children(item: &mut MenuItem) {
        item.children.sort_by(|a, b| {
            a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title))
        });
        for c in &mut item.children {
            sort_children(c);
        }
    }
    for r in &mut roots {
        sort_children(r);
    }

    // Root sortieren: explizite menu_order zuerst, Rest nach order/title.
    let order_index: BTreeMap<&String, usize> =
        order.iter().enumerate().map(|(i, s)| (s, i)).collect();
    roots.sort_by(|a, b| {
        let ai = order_index.get(&a.slug);
        let bi = order_index.get(&b.slug);
        match (ai, bi) {
            (Some(i), Some(j)) => i.cmp(j),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title)),
        }
    });
    roots
}

/// Sucht zum gegebenen Slug den nächsten *vorhandenen* Vorfahren im
/// Menü-Slug-Set. Liefert `None` für Root-Items oder wenn kein Vorfahre
/// im Menü ist (→ Knoten wird Root).
fn nearest_visible_ancestor(
    slug: &str,
    visible: &std::collections::HashSet<&str>,
) -> Option<String> {
    let mut s: &str = slug;
    while let Some(idx) = s.rfind('/') {
        let parent = &slug[..idx];
        s = parent;
        if visible.contains(parent) {
            return Some(parent.to_string());
        }
        // weiter hochlaufen
    }
    None
}

/// Walkt durch alle Blocks und ergänzt `content_html` für jeden `text`-Block
/// aus dessen `content`-Markdown. Innerhalb von `columns` rekursiv.
fn render_text_blocks(blocks: &mut [serde_json::Value]) {
    for block in blocks.iter_mut() {
        let Some(obj) = block.as_object_mut() else { continue };
        let kind = obj.get("type").and_then(|v| v.as_str()).map(String::from);
        match kind.as_deref() {
            Some("text") => {
                let md = obj.get("content").and_then(|v| v.as_str()).unwrap_or("");
                obj.insert("content_html".into(), serde_json::Value::String(render_markdown(md)));
            }
            Some("columns") => {
                if let Some(cols) = obj.get_mut("columns").and_then(|v| v.as_array_mut()) {
                    for col in cols.iter_mut() {
                        if let Some(inner) = col.as_array_mut() {
                            render_text_blocks(inner);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn page_url(slug: &str) -> String {
    if slug == "index" { "/".into() } else { format!("/{slug}/") }
}

fn render_page(
    tera: &Tera,
    site: &SiteCtx,
    page: &PageDoc,
    out_dir: &Utf8PathBuf,
    site_manifest: &projectfs::SiteManifest,
) -> Result<()> {
    if !theme_contract::is_valid_slug(&page.slug) {
        return Err(anyhow!("[BAD_SLUG] {}", page.slug));
    }
    let template = page
        .frontmatter
        .template
        .clone()
        .or_else(|| site_manifest.default_template.clone())
        .unwrap_or_else(|| if page.slug == "index" { "index.html".into() } else { "page.html".into() });
    let template = if template.ends_with(".html") { template } else { format!("{template}.html") };

    // Jeden text-Block (top-level + verschachtelt in Columns) zu HTML rendern.
    let mut blocks = page.frontmatter.blocks.clone();
    render_text_blocks(&mut blocks);

    let page_ctx = PageCtx {
        title: page.frontmatter.title.clone(),
        slug: page.slug.clone(),
        url: page_url(&page.slug),
        template: template.clone(),
        meta: page.frontmatter.meta.clone(),
        blocks,
    };

    let mut ctx = TeraContext::new();
    ctx.insert("site", site);
    ctx.insert("page", &page_ctx);
    ctx.insert("blocks", &page_ctx.blocks);

    let html = tera
        .render(&template, &ctx)
        .with_context(|| format!("render template {template} for page {}", page.slug))?;

    let out_path = if page.slug == "index" {
        out_dir.join("index.html")
    } else {
        let dir = out_dir.join(&page.slug);
        std::fs::create_dir_all(&dir)?;
        dir.join("index.html")
    };
    std::fs::write(&out_path, html)?;
    Ok(())
}

fn render_404(tera: &Tera, site: &SiteCtx, out_dir: &Utf8PathBuf) -> Result<()> {
    let mut ctx = TeraContext::new();
    ctx.insert("site", site);
    ctx.insert(
        "page",
        &serde_json::json!({ "title": "404", "slug": "404" }),
    );
    let html = tera.render("404.html", &ctx)?;
    std::fs::write(out_dir.join("404.html"), html)?;
    Ok(())
}

fn copy_dir(src: &Utf8PathBuf, dst: &Utf8PathBuf) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src).min_depth(1) {
        let entry = entry?;
        let rel = entry.path().strip_prefix(src.as_std_path()).unwrap();
        let target = dst.join(rel.to_string_lossy().as_ref());
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use projectfs::{MenuConfig, PageDoc, PageFrontmatter};

    fn page(slug: &str, title: &str, menu_show: bool, menu_order: Option<i32>, visible: bool) -> PageDoc {
        PageDoc {
            slug: slug.into(),
            frontmatter: PageFrontmatter {
                title: title.into(),
                template: None,
                visible,
                menu: MenuConfig { show: menu_show, order: menu_order },
                blocks: vec![],
                meta: Default::default(),
                favorite: false,
            },
            body_markdown: String::new(),
        }
    }

    #[test]
    fn page_url_index_is_root() {
        assert_eq!(page_url("index"), "/");
        assert_eq!(page_url("about"), "/about/");
        assert_eq!(page_url("blog-1"), "/blog-1/");
    }

    #[test]
    fn build_menu_filters_hidden_and_invisible() {
        let pages = vec![
            page("a", "A", true, Some(1), true),
            page("b", "B", false, None, true),     // menu.show=false → raus
            page("c", "C", true, Some(2), false),  // visible=false → raus
        ];
        let items = build_menu(&pages, &[]);
        let slugs: Vec<_> = items.iter().map(|i| i.slug.as_str()).collect();
        assert_eq!(slugs, vec!["a"]);
    }

    #[test]
    fn build_menu_honours_menu_order() {
        let pages = vec![
            page("about", "About", true, Some(10), true),
            page("index", "Home", true, Some(20), true),
            page("contact", "Contact", true, Some(5), true),
        ];
        let order = vec!["index".to_string(), "about".to_string()];
        let items = build_menu(&pages, &order);
        let slugs: Vec<_> = items.iter().map(|i| i.slug.as_str()).collect();
        // index + about kommen wie in menu_order zuerst, contact fällt hinten an
        assert_eq!(slugs, vec!["index", "about", "contact"]);
    }

    #[test]
    fn build_menu_fallback_sorts_by_order_then_title() {
        let pages = vec![
            page("zeta", "Zeta", true, Some(5), true),
            page("alpha", "Alpha", true, Some(5), true),
            page("beta", "Beta", true, Some(1), true),
        ];
        let items = build_menu(&pages, &[]);
        let slugs: Vec<_> = items.iter().map(|i| i.slug.as_str()).collect();
        // beta(order=1) zuerst, dann alpha(order=5) vor zeta(order=5) alphabetisch
        assert_eq!(slugs, vec!["beta", "alpha", "zeta"]);
    }

    #[test]
    fn build_menu_baut_baum_aus_pfad_slugs() {
        let pages = vec![
            page("about", "About", true, Some(10), true),
            page("about/team", "Team", true, Some(2), true),
            page("about/contact", "Contact", true, Some(1), true),
            page("index", "Home", true, Some(1), true),
        ];
        let items = build_menu(&pages, &[]);
        let root_slugs: Vec<_> = items.iter().map(|i| i.slug.as_str()).collect();
        assert_eq!(root_slugs, vec!["index", "about"]);

        let about = items.iter().find(|i| i.slug == "about").unwrap();
        let child_slugs: Vec<_> = about.children.iter().map(|c| c.slug.as_str()).collect();
        // Kinder nach order: contact (1) vor team (2)
        assert_eq!(child_slugs, vec!["about/contact", "about/team"]);
    }

    #[test]
    fn build_menu_haengt_an_naechsten_sichtbaren_vorfahren_wenn_parent_versteckt() {
        // about ist versteckt (menu.show=false), aber about/team ist sichtbar
        // → about/team rutscht auf Root, nicht verschwinden.
        let pages = vec![
            page("about", "About", true, None, false),       // im Menü nicht sichtbar
            page("about/team", "Team", true, None, true),
        ];
        let items = build_menu(&pages, &[]);
        let root_slugs: Vec<_> = items.iter().map(|i| i.slug.as_str()).collect();
        assert_eq!(root_slugs, vec!["about/team"]);
    }

    #[test]
    fn build_menu_dreistufige_hierarchie() {
        let pages = vec![
            page("a", "A", true, Some(1), true),
            page("a/b", "B", true, Some(1), true),
            page("a/b/c", "C", true, Some(1), true),
        ];
        let items = build_menu(&pages, &[]);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].slug, "a");
        assert_eq!(items[0].children.len(), 1);
        assert_eq!(items[0].children[0].slug, "a/b");
        assert_eq!(items[0].children[0].children.len(), 1);
        assert_eq!(items[0].children[0].children[0].slug, "a/b/c");
    }

    #[test]
    fn build_menu_missing_order_defaults_to_1000() {
        let pages = vec![
            page("a", "A", true, None, true),
            page("b", "B", true, Some(1), true),
        ];
        let items = build_menu(&pages, &[]);
        let slugs: Vec<_> = items.iter().map(|i| i.slug.as_str()).collect();
        assert_eq!(slugs, vec!["b", "a"]);
    }
}

fn write_css_vars(out_dir: &Utf8PathBuf, vars: &BTreeMap<String, String>) -> Result<()> {
    let mut s = String::from(":root {\n");
    for (k, v) in vars {
        s.push_str(&format!("  {k}: {v};\n"));
    }
    s.push_str("}\n");
    let path = out_dir.join("styles").join("_vars.css");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, s)?;
    Ok(())
}
