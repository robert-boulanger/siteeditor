import { useEffect, useState } from "react";
import { useStore } from "./store";
import type { Block, PageFrontmatter, ThemeInfo } from "./store";
import { open as openDialog, save as saveDialog, ask } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Plus, FolderOpen, FolderPlus, Save, Hammer, UploadCloud, Settings } from "lucide-react";
import { BlockList } from "./components/blocks/BlockList";
import { NewPageDialog } from "./components/NewPageDialog";
import { RenamePageDialog } from "./components/RenamePageDialog";
import { ThemeCssEditor } from "./components/ThemeCssEditor";
import { SettingsModal } from "./components/SettingsModal";
import { DeployModal } from "./components/DeployModal";
import { PageTree, buildPageTree } from "./components/PageTree";
import { SidebarSplitter, loadSidebarWidth } from "./components/SidebarSplitter";
import "./App.css";

function App() {
  const {
    project, currentPage, status, busy,
    bootstrap, open, loadPage, savePageFull, build,
    createPage, renamePage, deletePage,
    listThemes, setActiveTheme,
    movePage, setFavorite,
  } = useStore();

  const [draftFm, setDraftFm] = useState<PageFrontmatter | null>(null);
  const [showNewPage, setShowNewPage] = useState(false);
  const [renameSlug, setRenameSlug] = useState<string | null>(null);
  const [themes, setThemes] = useState<ThemeInfo[]>([]);
  const [cssEditorOpen, setCssEditorOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [deployOpen, setDeployOpen] = useState(false);

  useEffect(() => {
    const w = loadSidebarWidth();
    document.documentElement.style.setProperty("--sidebar-width", `${w}px`);
  }, []);

  useEffect(() => {
    if (!project) { setThemes([]); return; }
    listThemes().then(setThemes).catch(() => setThemes([]));
  }, [project?.root, listThemes]);

  async function handleThemeChange(slug: string) {
    if (!project || slug === project.active_theme) return;
    if (!(await confirmDirtyOrContinue())) return;
    try {
      await setActiveTheme(slug);
      const next = await listThemes();
      setThemes(next);
    } catch { /* status zeigt Fehler */ }
  }

  const dirty =
    currentPage != null &&
    draftFm != null &&
    JSON.stringify(draftFm) !== JSON.stringify(currentPage.frontmatter);

  useEffect(() => {
    setDraftFm(currentPage ? structuredClone(currentPage.frontmatter) : null);
  }, [currentPage?.slug, currentPage?.frontmatter]);

  async function confirmDirtyOrContinue(): Promise<boolean> {
    if (!dirty) return true;
    return await ask("Ungespeicherte Änderungen verwerfen?", {
      title: "Achtung",
      kind: "warning",
    });
  }

  async function handleSave() {
    if (!currentPage || !draftFm) return;
    // body bleibt leer — alle Texte stecken inzwischen in den text-Blocks.
    try {
      await savePageFull(currentPage.slug, draftFm, "");
      // Build hinterherjagen, damit der offene Preview-Tab via SSE neu lädt.
      await build();
    } catch { /* status zeigt Fehler */ }
  }

  async function handleLoadPage(slug: string) {
    if (!(await confirmDirtyOrContinue())) return;
    loadPage(slug);
  }

  async function pickProjectFolder() {
    const path = await openDialog({ directory: true, multiple: false, title: "Projekt-Ordner wählen" });
    if (typeof path === "string") await open(path);
  }

  async function pickBootstrapTarget() {
    const path = await saveDialog({
      title: "Neues Beispielprojekt — Ordnername wählen",
      defaultPath: "mein-siteeditor-projekt",
    });
    if (typeof path === "string") await bootstrap(path);
  }

  async function buildAndOpen() {
    try {
      await build();
      try {
        const url = await invoke<string>("preview_url");
        await invoke("open_in_browser", { path: url });
      } catch (e) {
        useStore.getState().setStatus(`Build OK, aber Browser-Öffnen fehlgeschlagen: ${e}`);
      }
    } catch { /* build-Fehler bereits in Status */ }
  }

  async function handleRename(oldSlug: string) {
    if (!(await confirmDirtyOrContinue())) return;
    setRenameSlug(oldSlug);
  }

  async function handleDelete(slug: string) {
    const ok = await ask(`Page "${slug}" wirklich löschen?`, {
      title: "Löschen bestätigen",
      kind: "warning",
    });
    if (!ok) return;
    try { await deletePage(slug); }
    catch { /* status */ }
  }

  async function handleToggleVisible(slug: string) {
    // Klein-Operation: Page laden, frontmatter.visible togglen, save.
    if (!project) return;
    try {
      const page = await invoke<{ frontmatter: PageFrontmatter }>("load_page", { slug });
      const fm = { ...page.frontmatter, visible: !page.frontmatter.visible };
      await savePageFull(slug, fm, "");
    } catch { /* status */ }
  }

  function updateFm<K extends keyof PageFrontmatter>(key: K, value: PageFrontmatter[K]) {
    setDraftFm((prev) => (prev ? { ...prev, [key]: value } : prev));
  }
  function updateMenu<K extends keyof PageFrontmatter["menu"]>(key: K, value: PageFrontmatter["menu"][K]) {
    setDraftFm((prev) => (prev ? { ...prev, menu: { ...prev.menu, [key]: value } } : prev));
  }
  function updateBlocks(blocks: Block[]) {
    setDraftFm((prev) => (prev ? { ...prev, blocks } : prev));
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <h1>siteeditor</h1>
        <div className="actions">
          <button
            className="icon-btn"
            onClick={pickBootstrapTarget}
            disabled={busy}
            title="Beispielprojekt erzeugen"
            aria-label="Beispielprojekt erzeugen"
          >
            <FolderPlus size={18} />
          </button>
          <button
            className="icon-btn"
            onClick={pickProjectFolder}
            disabled={busy}
            title="Projekt öffnen…"
            aria-label="Projekt öffnen"
          >
            <FolderOpen size={18} />
          </button>
          <span className="toolbar-sep" />
          <button
            className="icon-btn save-btn"
            onClick={handleSave}
            disabled={!dirty || busy}
            title={dirty ? "Speichern (ungespeicherte Änderungen)" : "Speichern (nichts zu tun)"}
            aria-label="Speichern"
          >
            <Save size={18} />
            {dirty && <span className="dirty-dot" aria-hidden>●</span>}
          </button>
          <button
            className="icon-btn"
            onClick={buildAndOpen}
            disabled={busy || !project}
            title="Build & Preview"
            aria-label="Build und Preview"
          >
            <Hammer size={18} />
          </button>
          <span className="toolbar-sep" />
          <button
            className="icon-btn"
            onClick={() => setDeployOpen(true)}
            disabled={busy || !project}
            title="Deploy (Site veröffentlichen)"
            aria-label="Deploy"
          >
            <UploadCloud size={18} />
          </button>
          <button
            className="icon-btn"
            onClick={() => setSettingsOpen(true)}
            disabled={busy || !project}
            title="Einstellungen (Deploy-Profile…)"
            aria-label="Einstellungen"
          >
            <Settings size={18} />
          </button>
        </div>
      </header>

      <div className="app-main">
        <aside className="sidebar">
          {project ? (
            <>
              <div className="project-meta">
                <strong>{project.title}</strong>
                <label className="theme-picker">
                  <span className="muted">Theme</span>
                  <select
                    value={project.active_theme}
                    disabled={busy || themes.length === 0}
                    onChange={(e) => handleThemeChange(e.currentTarget.value)}
                  >
                    {/* aktuelles Theme auch dann zeigen, wenn list_themes es nicht (mehr) kennt */}
                    {!themes.some((t) => t.slug === project.active_theme) && (
                      <option value={project.active_theme}>{project.active_theme}</option>
                    )}
                    {themes.map((t) => (
                      <option key={t.slug} value={t.slug}>
                        {t.display_name}
                        {t.slug !== t.display_name ? ` (${t.slug})` : ""}
                      </option>
                    ))}
                  </select>
                </label>
                <button
                  className="theme-css-btn"
                  onClick={() => setCssEditorOpen(true)}
                  disabled={busy}
                  title="CSS des aktiven Themes bearbeiten"
                >
                  Theme-CSS bearbeiten…
                </button>
                <div className="muted small">{project.root}</div>
              </div>
              <div className="pages-header">
                <h3>Pages</h3>
                <button className="add-page" title="Neue Page anlegen" onClick={async () => {
                  if (!(await confirmDirtyOrContinue())) return;
                  setShowNewPage(true);
                }}>
                  <Plus size={14} /> Neu
                </button>
              </div>
              {project.pages.some((p) => p.favorite) && (
                <>
                  <h3>Favoriten</h3>
                  <PageTree
                    flat
                    nodes={project.pages
                      .filter((p) => p.favorite)
                      .sort((a, b) => a.title.localeCompare(b.title))
                      .map((p) => ({ ...p, children: [], order: 0 }))}
                    currentSlug={currentPage?.slug}
                    onSelect={handleLoadPage}
                    onToggleVisible={handleToggleVisible}
                    onToggleFavorite={(slug, fav) => { setFavorite(slug, fav).catch(() => {}); }}
                    onRename={handleRename}
                    onDelete={handleDelete}
                    onMove={() => { /* in der Favoriten-Liste kein DnD */ }}
                  />
                  <h3>Alle Pages</h3>
                </>
              )}
              <PageTree
                nodes={buildPageTree(project.pages)}
                currentSlug={currentPage?.slug}
                onSelect={handleLoadPage}
                onToggleVisible={handleToggleVisible}
                onToggleFavorite={(slug, fav) => { setFavorite(slug, fav).catch(() => {}); }}
                onRename={handleRename}
                onDelete={handleDelete}
                onMove={async (slug, newParent, newOrder) => {
                  if (!(await confirmDirtyOrContinue())) return;
                  try { await movePage(slug, newParent, newOrder); }
                  catch { /* status zeigt Fehler */ }
                }}
              />
            </>
          ) : (
            <p className="muted">Kein Projekt geöffnet. Lege ein Beispielprojekt an oder öffne einen vorhandenen Ordner.</p>
          )}
        </aside>

        <SidebarSplitter onChange={() => { /* width lives in CSS var */ }} />

        <section className="main-pane">
          {currentPage && draftFm ? (
            <article>
              <div className="page-header-row">
                <h2>{draftFm.title}</h2>
              </div>

              <details className="page-meta" open>
                <summary>Page-Metadaten</summary>
                <div className="meta-grid">
                  <label>
                    <span>Titel</span>
                    <input value={draftFm.title} onChange={(e) => updateFm("title", e.currentTarget.value)} />
                  </label>
                  <label>
                    <span>Template</span>
                    <select value={draftFm.template ?? "page"} onChange={(e) => updateFm("template", e.currentTarget.value)}>
                      <option value="page">page</option>
                      <option value="index">index</option>
                    </select>
                  </label>
                  <label className="row">
                    <input type="checkbox" checked={draftFm.visible} onChange={(e) => updateFm("visible", e.currentTarget.checked)} />
                    <span>sichtbar</span>
                  </label>
                  <label className="row">
                    <input type="checkbox" checked={draftFm.menu.show} onChange={(e) => updateMenu("show", e.currentTarget.checked)} />
                    <span>im Menü</span>
                  </label>
                  <label>
                    <span>Menü-Reihenfolge</span>
                    <input
                      type="number"
                      value={draftFm.menu.order ?? 100}
                      onChange={(e) => updateMenu("order", Number(e.currentTarget.value))}
                    />
                  </label>
                </div>
              </details>

              <h4>Blocks</h4>
              <BlockList blocks={draftFm.blocks as Block[]} onChange={updateBlocks} />
            </article>
          ) : (
            <p className="muted">Keine Page ausgewählt.</p>
          )}
        </section>
      </div>

      <footer className="status-bar">{status}</footer>

      {renameSlug && project && (
        <RenamePageDialog
          oldSlug={renameSlug}
          existingSlugs={project.pages.map((p) => p.slug).filter((s) => s !== renameSlug)}
          onCancel={() => setRenameSlug(null)}
          onRename={async (next) => {
            await renamePage(renameSlug, next);
            setRenameSlug(null);
          }}
        />
      )}

      {cssEditorOpen && project && (
        <ThemeCssEditor
          slug={project.active_theme}
          onClose={() => setCssEditorOpen(false)}
        />
      )}

      {settingsOpen && project && (
        <SettingsModal onClose={() => setSettingsOpen(false)} />
      )}

      {deployOpen && project && (
        <DeployModal onClose={() => setDeployOpen(false)} />
      )}

      {showNewPage && project && (
        <NewPageDialog
          existingSlugs={project.pages.map((p) => p.slug)}
          onCancel={() => setShowNewPage(false)}
          onCreate={async (slug, fm) => {
            await createPage(slug, fm);
            setShowNewPage(false);
          }}
        />
      )}
    </div>
  );
}

export default App;
