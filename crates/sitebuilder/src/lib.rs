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
    content_html: String,
    content_markdown: String,
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
        tera.add_raw_template(&name, &raw)
            .with_context(|| format!("add template {name}"))?;
    }
    Ok(())
}

fn load_theme_manifest(theme_dir: &Utf8PathBuf) -> Result<theme_contract::ThemeManifest> {
    let raw = std::fs::read_to_string(theme_dir.join("theme.json"))?;
    Ok(serde_json::from_str(&raw)?)
}

fn build_menu(pages: &[PageDoc], order: &[String]) -> Vec<MenuItem> {
    let mut items: Vec<MenuItem> = pages
        .iter()
        .filter(|p| p.frontmatter.visible && p.frontmatter.menu.show)
        .map(|p| MenuItem {
            title: p.frontmatter.title.clone(),
            slug: p.slug.clone(),
            url: page_url(&p.slug),
            order: p.frontmatter.menu.order.unwrap_or(1000),
        })
        .collect();

    // honour menu_order: items in order come first, in given order; rest sorted by their order field, then title
    let order_index: BTreeMap<&String, usize> =
        order.iter().enumerate().map(|(i, s)| (s, i)).collect();
    items.sort_by(|a, b| {
        let ai = order_index.get(&a.slug);
        let bi = order_index.get(&b.slug);
        match (ai, bi) {
            (Some(i), Some(j)) => i.cmp(j),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.order.cmp(&b.order).then_with(|| a.title.cmp(&b.title)),
        }
    });
    items
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

    // text-block-Validator: 0 oder 1
    let text_count = page
        .frontmatter
        .blocks
        .iter()
        .filter(|b| b.get("type").and_then(|v| v.as_str()) == Some("text"))
        .count();
    if text_count > 1 {
        return Err(anyhow!("[MULTIPLE_TEXT_BLOCKS] page {}", page.slug));
    }

    let page_ctx = PageCtx {
        title: page.frontmatter.title.clone(),
        slug: page.slug.clone(),
        url: page_url(&page.slug),
        template: template.clone(),
        meta: page.frontmatter.meta.clone(),
        content_html: render_markdown(&page.body_markdown),
        content_markdown: page.body_markdown.clone(),
        blocks: page.frontmatter.blocks.clone(),
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
