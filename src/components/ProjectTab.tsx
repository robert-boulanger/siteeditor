import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ThemeInfo } from "../store";

type Props = {
  busy: boolean;
  setBusy: (b: boolean) => void;
  onSaved?: (s: SiteSettings) => void;
  onDirtyChange?: (dirty: boolean) => void;
};

export type SiteSettings = {
  title: string;
  description: string | null;
  base_url: string;
  active_theme: string;
  language: string | null;
};

export function ProjectTab({ busy, setBusy, onSaved, onDirtyChange }: Props) {
  const [initial, setInitial] = useState<SiteSettings | null>(null);
  const [draft, setDraft] = useState<SiteSettings | null>(null);
  const [themes, setThemes] = useState<ThemeInfo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [savedHint, setSavedHint] = useState(false);

  useEffect(() => {
    Promise.all([
      invoke<SiteSettings>("load_site_settings"),
      invoke<ThemeInfo[]>("list_themes"),
    ])
      .then(([s, t]) => {
        setInitial(s);
        setDraft({ ...s });
        setThemes(t);
      })
      .catch((e) => setError(String(e)));
  }, []);

  const dirty = !!(draft && initial && JSON.stringify(draft) !== JSON.stringify(initial));
  useEffect(() => { onDirtyChange?.(dirty); }, [dirty, onDirtyChange]);

  function update<K extends keyof SiteSettings>(k: K, v: SiteSettings[K]) {
    setDraft((d) => (d ? { ...d, [k]: v } : d));
    setSavedHint(false);
  }

  async function save() {
    if (!draft) return;
    setError(null);
    setBusy(true);
    try {
      const patch = {
        title: draft.title,
        description: draft.description,
        base_url: draft.base_url,
        active_theme: draft.active_theme,
        language: draft.language,
      };
      const saved = await invoke<SiteSettings>("save_site_settings", { patch });
      setInitial(saved);
      setDraft({ ...saved });
      setSavedHint(true);
      onSaved?.(saved);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  function revert() {
    if (initial) setDraft({ ...initial });
    setSavedHint(false);
    setError(null);
  }

  if (!draft) {
    return <div className="muted">Lade Projekt-Einstellungen…</div>;
  }

  return (
    <div className="settings-tab" style={{ display: "flex", flexDirection: "column", gap: "0.6rem", maxWidth: 520 }}>
      <label>
        <span>Titel</span>
        <input
          value={draft.title}
          onChange={(e) => update("title", e.currentTarget.value)}
          placeholder="Mein Projekt"
        />
      </label>

      <label>
        <span>Beschreibung</span>
        <input
          value={draft.description ?? ""}
          onChange={(e) => update("description", e.currentTarget.value || null)}
          placeholder="Optional"
        />
      </label>

      <label>
        <span>Basis-URL</span>
        <input
          value={draft.base_url}
          onChange={(e) => update("base_url", e.currentTarget.value)}
          placeholder="https://example.com"
        />
      </label>

      <label>
        <span>Sprache</span>
        <input
          value={draft.language ?? ""}
          onChange={(e) => update("language", e.currentTarget.value || null)}
          placeholder="de, en, de-DE …"
        />
      </label>

      <label>
        <span>Aktives Theme</span>
        <select
          value={draft.active_theme}
          onChange={(e) => update("active_theme", e.currentTarget.value)}
        >
          {themes.length === 0 && <option value={draft.active_theme}>{draft.active_theme}</option>}
          {themes.map((t) => (
            <option key={t.slug} value={t.slug}>{t.display_name || t.slug}</option>
          ))}
        </select>
      </label>

      {error && (
        <div className="modal-error" style={{ color: "var(--danger, #c00)" }}>{error}</div>
      )}

      {savedHint && !dirty && (
        <div className="muted small">Gespeichert.</div>
      )}

      <div style={{ display: "flex", gap: "0.5rem", justifyContent: "flex-end" }}>
        <button type="button" onClick={revert} disabled={busy || !dirty}>Verwerfen</button>
        <button type="button" onClick={save} disabled={busy || !dirty}>Speichern</button>
      </div>
    </div>
  );
}
