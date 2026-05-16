import { useEffect, useState } from "react";
import { useStore } from "./store";
import type { Block, PageFrontmatter } from "./store";
import { open as openDialog, save as saveDialog, ask } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { Eye, EyeOff, Pencil, Trash2, Plus } from "lucide-react";
import { BlockList } from "./components/blocks/BlockList";
import { NewPageDialog } from "./components/NewPageDialog";
import "./App.css";

function App() {
  const {
    project, currentPage, status, busy,
    bootstrap, open, loadPage, savePageFull, build,
    createPage, renamePage, deletePage,
  } = useStore();

  const [draftFm, setDraftFm] = useState<PageFrontmatter | null>(null);
  const [showNewPage, setShowNewPage] = useState(false);

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
    try { await savePageFull(currentPage.slug, draftFm, ""); }
    catch { /* status zeigt Fehler */ }
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
    const next = window.prompt(`Neuer Slug für "${oldSlug}":`, oldSlug);
    if (!next || next === oldSlug) return;
    try { await renamePage(oldSlug, next); }
    catch { /* status */ }
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
          <button onClick={pickBootstrapTarget} disabled={busy}>Beispielprojekt erzeugen</button>
          <button onClick={pickProjectFolder} disabled={busy}>Projekt öffnen…</button>
          <button onClick={buildAndOpen} disabled={busy || !project}>Build &amp; Preview</button>
        </div>
      </header>

      <div className="app-main">
        <aside className="sidebar">
          {project ? (
            <>
              <div className="project-meta">
                <strong>{project.title}</strong>
                <div className="muted">Theme: {project.active_theme}</div>
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
              <ul className="page-list">
                {project.pages.map((p) => (
                  <li key={p.slug} className={currentPage?.slug === p.slug ? "is-active" : ""}>
                    <button
                      className="page-select"
                      onClick={() => handleLoadPage(p.slug)}
                    >
                      <span className="slug">{p.slug}</span>
                      <span className="title">{p.title}</span>
                      {!p.visible && <span className="badge">hidden</span>}
                    </button>
                    <span className="page-actions">
                      <button
                        title={p.visible ? "Page ausblenden" : "Page einblenden"}
                        onClick={() => handleToggleVisible(p.slug)}
                      >
                        {p.visible ? <Eye size={14} /> : <EyeOff size={14} />}
                      </button>
                      <button title="Umbenennen" onClick={() => handleRename(p.slug)}>
                        <Pencil size={14} />
                      </button>
                      <button title="Löschen" onClick={() => handleDelete(p.slug)}>
                        <Trash2 size={14} />
                      </button>
                    </span>
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <p className="muted">Kein Projekt geöffnet. Lege ein Beispielprojekt an oder öffne einen vorhandenen Ordner.</p>
          )}
        </aside>

        <section className="main-pane">
          {currentPage && draftFm ? (
            <article>
              <div className="page-header-row">
                <h2>{draftFm.title}</h2>
                <div className="body-actions">
                  {dirty && <span className="dirty-dot" title="Ungespeicherte Änderungen">●</span>}
                  <button onClick={handleSave} disabled={!dirty || busy}>Speichern</button>
                </div>
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
