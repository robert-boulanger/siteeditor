import { useState } from "react";
import type { PageFrontmatter } from "../store";

type Props = {
  existingSlugs: string[];
  onCancel: () => void;
  onCreate: (slug: string, fm: PageFrontmatter) => Promise<void>;
};

const SLUG_RE = /^[a-z0-9]+(-[a-z0-9]+)*$/;

export function NewPageDialog({ existingSlugs, onCancel, onCreate }: Props) {
  const [title, setTitle] = useState("");
  const [slug, setSlug] = useState("");
  const [template, setTemplate] = useState("page");
  const [visible, setVisible] = useState(true);
  const [menuShow, setMenuShow] = useState(true);
  const [menuOrder, setMenuOrder] = useState(100);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!SLUG_RE.test(slug)) {
      setError("Slug muss kebab-case sein (a-z, 0-9, Bindestrich).");
      return;
    }
    if (existingSlugs.includes(slug)) {
      setError("Slug existiert bereits.");
      return;
    }
    if (!title.trim()) {
      setError("Titel darf nicht leer sein.");
      return;
    }
    setBusy(true);
    try {
      await onCreate(slug, {
        title: title.trim(),
        template: template || null,
        visible,
        menu: { show: menuShow, order: menuOrder },
        blocks: [],
        meta: {},
      });
    } catch (err: any) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  function suggestSlug(t: string) {
    return t.toLowerCase()
      .replace(/[äöüß]/g, (c) => ({ ä: "ae", ö: "oe", ü: "ue", ß: "ss" })[c] ?? c)
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "");
  }

  return (
    <div className="modal-backdrop" onMouseDown={onCancel}>
      <form className="modal" onMouseDown={(e) => e.stopPropagation()} onSubmit={submit}>
        <h3>Neue Page</h3>
        <label>
          <span>Titel</span>
          <input
            autoFocus
            value={title}
            onChange={(e) => {
              setTitle(e.currentTarget.value);
              if (!slug) setSlug(suggestSlug(e.currentTarget.value));
            }}
          />
        </label>
        <label>
          <span>Slug</span>
          <input value={slug} onChange={(e) => setSlug(e.currentTarget.value)} placeholder="ueber-uns" />
        </label>
        <label>
          <span>Template</span>
          <select value={template} onChange={(e) => setTemplate(e.currentTarget.value)}>
            <option value="page">page</option>
            <option value="index">index</option>
          </select>
        </label>
        <label className="row">
          <input type="checkbox" checked={visible} onChange={(e) => setVisible(e.currentTarget.checked)} />
          <span>sichtbar</span>
        </label>
        <label className="row">
          <input type="checkbox" checked={menuShow} onChange={(e) => setMenuShow(e.currentTarget.checked)} />
          <span>im Menü zeigen</span>
        </label>
        <label>
          <span>Menü-Reihenfolge</span>
          <input type="number" value={menuOrder} onChange={(e) => setMenuOrder(Number(e.currentTarget.value))} />
        </label>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <button type="button" onClick={onCancel} disabled={busy}>Abbrechen</button>
          <button type="submit" disabled={busy}>Anlegen</button>
        </div>
      </form>
    </div>
  );
}
