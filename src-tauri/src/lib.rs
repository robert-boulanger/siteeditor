mod bootstrap;
mod deploy_commands;
mod keystore;
mod preview;

use camino::Utf8PathBuf;
use projectfs::{AssetInfo, PageFrontmatter, Project, ThemeInfo};
use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};
use tauri::Manager;

#[derive(Default)]
struct AppState {
    project: Mutex<Option<Project>>,
    preview: preview::PreviewState,
    preview_port: OnceLock<u16>,
}

#[derive(Serialize)]
struct PageSummary {
    slug: String,
    title: String,
    visible: bool,
    template: Option<String>,
    menu_order: Option<i32>,
    favorite: bool,
}

#[derive(Serialize)]
struct OpenResult {
    root: String,
    title: String,
    active_theme: String,
    pages: Vec<PageSummary>,
}

#[derive(Serialize)]
struct BuildResult {
    pages_rendered: usize,
    output_dir: String,
    index_file: String,
}

fn to_str_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn bootstrap_example(state: tauri::State<'_, AppState>, path: String) -> Result<OpenResult, String> {
    let p = Utf8PathBuf::from(path);
    bootstrap::bootstrap_example_project(&p).map_err(to_str_err)?;
    let res = open_project_inner(p.clone())?;
    let project = Project::open(&p).map_err(to_str_err)?;
    state.preview.set_root(project.build_dir());
    *state.project.lock().unwrap() = Some(project);
    Ok(res)
}

#[tauri::command]
fn open_project(state: tauri::State<'_, AppState>, path: String) -> Result<OpenResult, String> {
    let res = open_project_inner(Utf8PathBuf::from(path))?;
    let p = Project::open(Utf8PathBuf::from(&res.root)).map_err(to_str_err)?;
    state.preview.set_root(p.build_dir());
    *state.project.lock().unwrap() = Some(p);
    Ok(res)
}

fn open_project_inner(path: Utf8PathBuf) -> Result<OpenResult, String> {
    let project = Project::open(&path).map_err(to_str_err)?;
    let pages = project
        .list_pages()
        .map_err(to_str_err)?
        .into_iter()
        .map(|p| PageSummary {
            slug: p.slug,
            title: p.frontmatter.title,
            visible: p.frontmatter.visible,
            template: p.frontmatter.template,
            menu_order: p.frontmatter.menu.order,
            favorite: p.frontmatter.favorite,
        })
        .collect();
    Ok(OpenResult {
        root: project.root.to_string(),
        title: project.manifest.title.clone(),
        active_theme: project.manifest.active_theme.clone(),
        pages,
    })
}

#[tauri::command]
fn load_page(state: tauri::State<'_, AppState>, slug: String) -> Result<serde_json::Value, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    let page = project.load_page(&slug).map_err(to_str_err)?;
    serde_json::to_value(&page).map_err(to_str_err)
}

#[derive(Deserialize)]
struct SavePageFullArgs {
    slug: String,
    frontmatter: PageFrontmatter,
    body: String,
}

#[tauri::command]
fn save_page_full(
    state: tauri::State<'_, AppState>,
    args: SavePageFullArgs,
) -> Result<serde_json::Value, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project
        .save_page_full(&args.slug, &args.frontmatter, &args.body)
        .map_err(to_str_err)?;
    let page = project.load_page(&args.slug).map_err(to_str_err)?;
    serde_json::to_value(&page).map_err(to_str_err)
}

#[derive(Deserialize)]
struct CreatePageArgs {
    slug: String,
    frontmatter: PageFrontmatter,
    #[serde(default)]
    body: String,
}

#[tauri::command]
fn create_page(
    state: tauri::State<'_, AppState>,
    args: CreatePageArgs,
) -> Result<Vec<PageSummary>, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project
        .create_page(&args.slug, &args.frontmatter, &args.body)
        .map_err(to_str_err)?;
    list_page_summaries(project)
}

#[tauri::command]
fn rename_page(
    state: tauri::State<'_, AppState>,
    old_slug: String,
    new_slug: String,
) -> Result<Vec<PageSummary>, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project.rename_page(&old_slug, &new_slug).map_err(to_str_err)?;
    list_page_summaries(project)
}

#[tauri::command]
fn delete_page(
    state: tauri::State<'_, AppState>,
    slug: String,
) -> Result<Vec<PageSummary>, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project.delete_page(&slug).map_err(to_str_err)?;
    list_page_summaries(project)
}

fn list_page_summaries(project: &Project) -> Result<Vec<PageSummary>, String> {
    Ok(project
        .list_pages()
        .map_err(to_str_err)?
        .into_iter()
        .map(|p| PageSummary {
            slug: p.slug,
            title: p.frontmatter.title,
            visible: p.frontmatter.visible,
            template: p.frontmatter.template,
            menu_order: p.frontmatter.menu.order,
            favorite: p.frontmatter.favorite,
        })
        .collect())
}

#[tauri::command]
fn list_assets(state: tauri::State<'_, AppState>) -> Result<Vec<AssetInfo>, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project.list_assets().map_err(to_str_err)
}

#[tauri::command]
fn import_asset(
    state: tauri::State<'_, AppState>,
    source: String,
) -> Result<String, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project
        .import_asset(Utf8PathBuf::from(source))
        .map_err(to_str_err)
}

#[tauri::command]
fn delete_asset(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project.delete_asset(&path).map_err(to_str_err)
}

#[tauri::command]
fn read_asset_data_url(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<String, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    // Pfad-Sicherheit über delete_asset's Logik — read braucht eigene Validierung.
    if path.is_empty() || path.contains("..") || path.starts_with('/') || path.contains(':') {
        return Err("unsicherer asset-pfad".into());
    }
    let dir = project.assets_dir();
    let target = dir.join(&path);
    let dir_canon = std::fs::canonicalize(&dir).map_err(to_str_err)?;
    let target_canon = std::fs::canonicalize(&target).map_err(to_str_err)?;
    if !target_canon.starts_with(&dir_canon) {
        return Err("pfad verlässt asset-verzeichnis".into());
    }
    let bytes = std::fs::read(&target_canon).map_err(to_str_err)?;
    let mime = guess_mime_for(&path);
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

fn guess_mime_for(path: &str) -> &'static str {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

#[tauri::command]
fn set_favorite(
    state: tauri::State<'_, AppState>,
    slug: String,
    favorite: bool,
) -> Result<Vec<PageSummary>, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project.set_favorite(&slug, favorite).map_err(to_str_err)?;
    list_page_summaries(project)
}

#[derive(Deserialize)]
struct MovePageArgs {
    slug: String,
    #[serde(default)]
    new_parent: Option<String>,
    #[serde(default)]
    new_order: Option<i32>,
}

#[tauri::command]
fn move_page(
    state: tauri::State<'_, AppState>,
    args: MovePageArgs,
) -> Result<Vec<PageSummary>, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project
        .move_page(&args.slug, args.new_parent.as_deref(), args.new_order)
        .map_err(to_str_err)?;
    list_page_summaries(project)
}

#[tauri::command]
fn list_themes(state: tauri::State<'_, AppState>) -> Result<Vec<ThemeInfo>, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project.list_installed_themes().map_err(to_str_err)
}

#[tauri::command]
fn set_active_theme(
    state: tauri::State<'_, AppState>,
    slug: String,
) -> Result<String, String> {
    let mut guard = state.project.lock().unwrap();
    let project = guard.as_mut().ok_or_else(|| "no project open".to_string())?;
    project.set_active_theme(&slug).map_err(to_str_err)?;
    Ok(project.manifest.active_theme.clone())
}

#[tauri::command]
fn read_theme_css(state: tauri::State<'_, AppState>, slug: String) -> Result<String, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project.read_theme_css(&slug).map_err(to_str_err)
}

#[tauri::command]
fn write_theme_css(
    state: tauri::State<'_, AppState>,
    slug: String,
    content: String,
) -> Result<(), String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    project.write_theme_css(&slug, &content).map_err(to_str_err)
}

#[tauri::command]
fn build_site(state: tauri::State<'_, AppState>) -> Result<BuildResult, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    let report = sitebuilder::build_site(project).map_err(to_str_err)?;
    state.preview.set_root(project.build_dir());
    state.preview.notify_reload();
    let index_file = format!("{}/index.html", report.output_dir);
    Ok(BuildResult {
        pages_rendered: report.pages_rendered,
        output_dir: report.output_dir,
        index_file,
    })
}

#[tauri::command]
fn preview_url(state: tauri::State<'_, AppState>) -> Result<String, String> {
    let port = state
        .preview_port
        .get()
        .ok_or_else(|| "preview server nicht initialisiert".to_string())?;
    Ok(format!("http://127.0.0.1:{port}/"))
}

#[tauri::command]
fn open_in_browser(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(&path).status();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd").args(["/C", "start", "", &path]).status();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(&path).status();

    match result {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("opener exited with status {s}")),
        Err(e) => Err(format!("opener failed: {e}")),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let state = app.state::<AppState>();
            let preview = state.preview.clone();
            let port_slot = &state.preview_port;
            // axum on the tauri async runtime
            let port = tauri::async_runtime::block_on(preview::start(preview))
                .map_err(|e| format!("preview server: {e}"))?;
            let _ = port_slot.set(port);
            eprintln!("preview server: http://127.0.0.1:{port}");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_version,
            bootstrap_example,
            open_project,
            load_page,
            save_page_full,
            create_page,
            rename_page,
            delete_page,
            move_page,
            set_favorite,
            list_assets,
            import_asset,
            delete_asset,
            read_asset_data_url,
            list_themes,
            set_active_theme,
            read_theme_css,
            write_theme_css,
            build_site,
            preview_url,
            open_in_browser,
            deploy_commands::deploy_list_profiles,
            deploy_commands::deploy_save_profile,
            deploy_commands::deploy_delete_profile,
            deploy_commands::deploy_has_secret,
            deploy_commands::deploy_dry_run,
            deploy_commands::deploy_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
