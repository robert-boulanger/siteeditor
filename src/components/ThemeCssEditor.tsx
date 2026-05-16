import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useStore } from "../store";

/** Modaler CSS-Editor für `themes/<slug>/styles/main.css`.
 *  Plain textarea, monospace — Syntax-Highlighting kommt nach, wenn nötig.
 *  Speichern triggert Build, SSE-Reload zeigt das Ergebnis im Preview-Tab. */
export function ThemeCssEditor({ slug, onClose }: { slug: string; onClose: () => void }) {
  const build = useStore((s) => s.build);
  const setStatus = useStore((s) => s.setStatus);
  const [original, setOriginal] = useState<string | null>(null);
  const [draft, setDraft] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const dirty = original != null && draft !== original;

  useEffect(() => {
    let cancelled = false;
    invoke<string>("read_theme_css", { slug })
      .then((css) => { if (!cancelled) { setOriginal(css); setDraft(css); } })
      .catch((e) => { if (!cancelled) setErr(String(e)); });
    return () => { cancelled = true; };
  }, [slug]);

  async function handleSave() {
    setBusy(true);
    setErr(null);
    try {
      await invoke("write_theme_css", { slug, content: draft });
      setOriginal(draft);
      try { await build(); } catch { /* status zeigt Build-Fehler */ }
      setStatus(`Theme-CSS gespeichert: ${slug}`);
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  }

  function handleClose() {
    if (dirty && !confirm("Ungespeicherte CSS-Änderungen verwerfen?")) return;
    onClose();
  }

  return (
    <div className="modal-backdrop" onClick={handleClose}>
      <div className="modal css-editor-modal" onClick={(e) => e.stopPropagation()}>
        <header className="modal-header">
          <h3>Theme-CSS bearbeiten — <code>{slug}/styles/main.css</code></h3>
          <button onClick={handleClose} title="Schließen">×</button>
        </header>
        {err && <div className="modal-error">{err}</div>}
        {original == null && !err && <div className="muted" style={{ padding: "1rem" }}>Lade…</div>}
        {original != null && (
          <textarea
            className="css-editor-textarea"
            value={draft}
            spellCheck={false}
            onChange={(e) => setDraft(e.currentTarget.value)}
          />
        )}
        <footer className="modal-footer">
          <span className="muted small">
            {dirty ? "● ungespeichert" : "gespeichert"}
            {original != null && ` — ${draft.length.toLocaleString("de-DE")} Zeichen`}
          </span>
          <span className="spacer" />
          <button onClick={handleClose} disabled={busy}>Schließen</button>
          <button
            className="save-btn"
            onClick={handleSave}
            disabled={!dirty || busy || original == null}
          >
            {busy ? "Speichere…" : "Speichern & Build"}
          </button>
        </footer>
      </div>
    </div>
  );
}
