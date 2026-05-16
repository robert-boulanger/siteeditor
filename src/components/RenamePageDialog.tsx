import { useState } from "react";

type Props = {
  oldSlug: string;
  existingSlugs: string[];
  onCancel: () => void;
  onRename: (newSlug: string) => Promise<void>;
};

const SLUG_RE = /^[a-z0-9]+(-[a-z0-9]+)*$/;

export function RenamePageDialog({ oldSlug, existingSlugs, onCancel, onRename }: Props) {
  const [slug, setSlug] = useState(oldSlug);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (slug === oldSlug) {
      onCancel();
      return;
    }
    if (!SLUG_RE.test(slug)) {
      setError("Slug muss kebab-case sein (a-z, 0-9, Bindestrich).");
      return;
    }
    if (existingSlugs.includes(slug)) {
      setError("Slug existiert bereits.");
      return;
    }
    setBusy(true);
    try {
      await onRename(slug);
    } catch (err: any) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="modal-backdrop" onMouseDown={onCancel}>
      <form className="modal" onMouseDown={(e) => e.stopPropagation()} onSubmit={submit}>
        <h3>Page umbenennen</h3>
        <p className="muted small">Alter Slug: <code>{oldSlug}</code></p>
        <label>
          <span>Neuer Slug</span>
          <input
            autoFocus
            value={slug}
            onChange={(e) => setSlug(e.currentTarget.value)}
            placeholder="ueber-uns"
          />
        </label>
        {error && <p className="error">{error}</p>}
        <div className="modal-actions">
          <button type="button" onClick={onCancel} disabled={busy}>Abbrechen</button>
          <button type="submit" disabled={busy}>Umbenennen</button>
        </div>
      </form>
    </div>
  );
}
