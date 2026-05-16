import { useStore } from "./store";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const { project, currentPage, status, busy, bootstrap, open, loadPage, build } = useStore();

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
      const r = await build();
      try {
        await invoke("open_in_browser", { path: r.index_file });
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
                      onClick={() => loadPage(p.slug)}
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

              <h4>Body (Markdown)</h4>
              <pre className="markdown">{currentPage.body_markdown}</pre>
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
