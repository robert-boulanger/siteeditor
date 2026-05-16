mod bootstrap;
mod preview;

use camino::Utf8PathBuf;
use projectfs::{PageFrontmatter, Project};
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
        })
        .collect())
}

#[tauri::command]
fn build_site(state: tauri::State<'_, AppState>) -> Result<BuildResult, String> {
    let guard = state.project.lock().unwrap();
    let project = guard.as_ref().ok_or_else(|| "no project open".to_string())?;
    let report = sitebuilder::build_site(project).map_err(to_str_err)?;
    state.preview.set_root(project.build_dir());
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
            build_site,
            preview_url,
            open_in_browser,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
