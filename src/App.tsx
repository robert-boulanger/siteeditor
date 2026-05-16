import { useEffect, useState } from "react";
import { useStore } from "./store";
import { open as openDialog, save as saveDialog, ask } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const { project, currentPage, status, busy, bootstrap, open, loadPage, savePageBody, build } = useStore();
  const [draft, setDraft] = useState<string>("");
  const dirty = currentPage ? draft !== currentPage.body_markdown : false;

  useEffect(() => {
    setDraft(currentPage?.body_markdown ?? "");
  }, [currentPage?.slug, currentPage?.body_markdown]);

  async function handleSave() {
    if (!currentPage) return;
    try {
      await savePageBody(currentPage.slug, draft);
    } catch {
      /* status zeigt Fehler */
    }
  }

  async function handleLoadPage(slug: string) {
    if (dirty) {
      const ok = await ask("Ungespeicherte Änderungen verwerfen?", {
        title: "Seitenwechsel",
        kind: "warning",
      });
      if (!ok) return;
    }
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
    } catch {
      /* build-Fehler ist bereits in der Status-Bar */
    }
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
              <h3>Pages</h3>
              <ul className="page-list">
                {project.pages.map((p) => (
                  <li key={p.slug}>
                    <button
                      className={currentPage?.slug === p.slug ? "is-active" : ""}
                      onClick={() => handleLoadPage(p.slug)}
                    >
                      <span className="slug">{p.slug}</span>
                      <span className="title">{p.title}</span>
                      {!p.visible && <span className="badge">hidden</span>}
                    </button>
                  </li>
                ))}
              </ul>
            </>
          ) : (
            <p className="muted">Kein Projekt geöffnet. Lege ein Beispielprojekt an oder öffne einen vorhandenen Ordner.</p>
          )}
        </aside>

        <section className="main-pane">
          {currentPage ? (
            <article>
              <h2>{currentPage.frontmatter.title}</h2>
              <p className="muted small">
                slug: <code>{currentPage.slug}</code> · template:{" "}
                <code>{currentPage.frontmatter.template ?? "(default)"}</code> · blocks:{" "}
                {currentPage.frontmatter.blocks.length}
              </p>

              <h4>Blocks (Frontmatter)</h4>
              <pre className="json">{JSON.stringify(currentPage.frontmatter.blocks, null, 2)}</pre>

              <div className="body-header">
                <h4>Body (Markdown)</h4>
                <div className="body-actions">
                  {dirty && <span className="dirty-dot" title="Ungespeicherte Änderungen">●</span>}
                  <button onClick={handleSave} disabled={!dirty || busy}>Speichern</button>
                </div>
              </div>
              <textarea
                className="body-editor"
                value={draft}
                onChange={(e) => setDraft(e.currentTarget.value)}
                spellCheck={false}
              />
            </article>
          ) : (
            <p className="muted">Keine Page ausgewählt.</p>
          )}
        </section>
      </div>

      <footer className="status-bar">{status}</footer>
    </div>
  );
}

export default App;
