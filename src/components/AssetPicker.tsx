import { useEffect, useState } from "react";
import { open as openDialog, ask } from "@tauri-apps/plugin-dialog";
import { useStore, type AssetInfo } from "../store";

type Props = {
  mode?: "single" | "multi";
  /** Optionaler Filter — z.B. nur Bilder. Default: alle. */
  accept?: "image" | "video" | "any";
  onCancel: () => void;
  onPick: (paths: string[]) => void;
};

const IMAGE_MIMES = ["image/png", "image/jpeg", "image/gif", "image/webp", "image/svg+xml", "image/avif"];
const VIDEO_MIMES = ["video/mp4", "video/webm", "video/quicktime"];

export function AssetPicker({ mode = "single", accept = "any", onCancel, onPick }: Props) {
  const [assets, setAssets] = useState<AssetInfo[]>([]);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const { listAssets, importAsset, deleteAsset, readAssetDataUrl } = useStore();

  async function refresh() {
    setError(null);
    try {
      const all = await listAssets();
      const filtered = all.filter((a) =>
        accept === "image" ? IMAGE_MIMES.includes(a.mime)
        : accept === "video" ? VIDEO_MIMES.includes(a.mime)
        : true,
      );
      setAssets(filtered);
      // Thumbnails nur für Bilder, lazy parallel laden
      const imgs = filtered.filter((a) => IMAGE_MIMES.includes(a.mime));
      const next: Record<string, string> = {};
      await Promise.all(imgs.map(async (a) => {
        try { next[a.path] = await readAssetDataUrl(a.path); } catch { /* ignore */ }
      }));
      setThumbs(next);
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => { refresh(); /* eslint-disable-next-line react-hooks/exhaustive-deps */ }, []);

  async function handleImport() {
    const extFilters =
      accept === "image" ? [{ name: "Bilder", extensions: ["png", "jpg", "jpeg", "gif", "webp", "svg", "avif"] }]
      : accept === "video" ? [{ name: "Videos", extensions: ["mp4", "webm", "mov"] }]
      : undefined;
    const result = await openDialog({ multiple: true, filters: extFilters, title: "Dateien importieren" });
    if (!result) return;
    const sources = Array.isArray(result) ? result : [result];
    setBusy(true);
    try {
      for (const src of sources) await importAsset(src);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleDelete(path: string) {
    const ok = await ask(`Asset "${path}" wirklich löschen?`, { title: "Löschen bestätigen", kind: "warning" });
    if (!ok) return;
    setBusy(true);
    try {
      await deleteAsset(path);
      setSelected((s) => { const n = new Set(s); n.delete(path); return n; });
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  function toggle(path: string) {
    setSelected((s) => {
      if (mode === "single") return new Set([path]);
      const n = new Set(s);
      if (n.has(path)) n.delete(path); else n.add(path);
      return n;
    });
  }

  function confirm() {
    const paths = assets.filter((a) => selected.has(a.path)).map((a) => `/assets/${a.path}`);
    if (paths.length === 0) return;
    onPick(paths);
  }

  const needle = filter.trim().toLowerCase();
  const visible = needle
    ? assets.filter((a) => a.name.toLowerCase().includes(needle) || a.path.toLowerCase().includes(needle))
    : assets;

  return (
    <div className="modal-backdrop" onMouseDown={onCancel}>
      <div className="modal asset-picker" onMouseDown={(e) => e.stopPropagation()}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
          <h3>Asset auswählen{mode === "multi" ? " (mehrere)" : ""}</h3>
          <button type="button" onClick={handleImport} disabled={busy}>+ Datei hinzufügen</button>
        </div>
        {error && <p className="error">{error}</p>}
        {assets.length > 0 && (
          <input
            type="search"
            className="asset-filter"
            placeholder="Filter nach Dateiname…"
            value={filter}
            onChange={(e) => setFilter(e.currentTarget.value)}
            autoFocus
          />
        )}
        {assets.length === 0 ? (
          <p style={{ opacity: 0.6 }}>Keine Assets vorhanden. Lade welche per „+ Datei hinzufügen" hoch.</p>
        ) : visible.length === 0 ? (
          <p style={{ opacity: 0.6 }}>Keine Treffer für „{filter}".</p>
        ) : (
          <div className="asset-grid">
            {visible.map((a) => {
              const isImg = IMAGE_MIMES.includes(a.mime);
              const sel = selected.has(a.path);
              return (
                <div key={a.path} className={`asset-tile${sel ? " selected" : ""}`} onClick={() => toggle(a.path)}>
                  <div className="asset-thumb">
                    {isImg && thumbs[a.path]
                      ? <img src={thumbs[a.path]} alt={a.name} />
                      : <span className="asset-icon">{a.mime.startsWith("video/") ? "🎬" : "📄"}</span>}
                  </div>
                  <div className="asset-meta">
                    <div className="asset-name" title={a.path}>{a.name}</div>
                    <div className="asset-size">{formatSize(a.size)}</div>
                  </div>
                  <button
                    type="button" className="asset-delete" title="Löschen"
                    onClick={(e) => { e.stopPropagation(); handleDelete(a.path); }}
                  >×</button>
                </div>
              );
            })}
          </div>
        )}
        <div className="modal-actions">
          <button type="button" onClick={onCancel}>Abbrechen</button>
          <button type="button" onClick={confirm} disabled={selected.size === 0}>
            Übernehmen{selected.size > 0 ? ` (${selected.size})` : ""}
          </button>
        </div>
      </div>
    </div>
  );
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} kB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
