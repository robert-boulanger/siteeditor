import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { DeployProfile, DiffReportDto, ProgressPayload } from "./deployTypes";
import { PROGRESS_EVENT } from "./deployTypes";

type Props = { onClose: () => void };

type LogEntry = { text: string; level: "info" | "error" };

function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

export function DeployModal({ onClose }: Props) {
  const [profiles, setProfiles] = useState<DeployProfile[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [hasSecret, setHasSecret] = useState<boolean | null>(null);
  const [diff, setDiff] = useState<DiffReportDto | null>(null);
  const [log, setLog] = useState<LogEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<DeployProfile[]>("deploy_list_profiles")
      .then((list) => {
        setProfiles(list);
        if (list.length > 0) setSelected(list[0].name);
      })
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    if (!selected) { setHasSecret(null); return; }
    invoke<boolean>("deploy_has_secret", { name: selected }).then(setHasSecret).catch(() => setHasSecret(null));
    setDiff(null);
    setLog([]);
    setError(null);
  }, [selected]);

  useEffect(() => {
    let off: UnlistenFn | null = null;
    listen<ProgressPayload>(PROGRESS_EVENT, (e) => {
      const ev = e.payload;
      let line: LogEntry;
      switch (ev.kind) {
        case "connected": line = { text: "Verbunden", level: "info" }; break;
        case "diff_resolved":
          line = { text: `Diff: ${ev.upload_count} Dateien · ${fmtBytes(ev.upload_bytes)}`, level: "info" }; break;
        case "file_start": line = { text: `→ ${ev.rel_path} (${fmtBytes(ev.size)})`, level: "info" }; break;
        case "file_done": line = { text: `✓ ${ev.rel_path}`, level: "info" }; break;
        case "manifest_written": line = { text: "Manifest geschrieben", level: "info" }; break;
        case "done":
          line = { text: `Fertig — ${ev.uploaded} Dateien (${fmtBytes(ev.total_bytes)})`, level: "info" };
          setBusy(false);
          break;
        case "error":
          line = { text: `Fehler: ${ev.message}`, level: "error" };
          setError(ev.message);
          setBusy(false);
          break;
      }
      setLog((l) => [...l, line]);
    }).then((unlisten) => { off = unlisten; });
    return () => { if (off) off(); };
  }, []);

  async function dryRun() {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setDiff(null);
    try {
      const r = await invoke<DiffReportDto>("deploy_dry_run", { name: selected });
      setDiff(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function deploy() {
    if (!selected) return;
    setBusy(true);
    setError(null);
    setLog([{ text: `Starte Deploy für "${selected}"…`, level: "info" }]);
    try {
      await invoke("deploy_run", { name: selected });
      // Done-Event kommt asynchron — busy wird im Listener gelöst.
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  const blockedNoSecret = hasSecret === false;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()} style={{ minWidth: 560, maxHeight: "85vh", display: "flex", flexDirection: "column" }}>
        <h3>Deploy</h3>

        {profiles.length === 0 ? (
          <p className="muted">Keine Deploy-Profile angelegt. Lege eines im Einstellungen-Dialog (Zahnrad) an.</p>
        ) : (
          <>
            <label>
              <span>Profil</span>
              <select value={selected ?? ""} onChange={(e) => setSelected(e.currentTarget.value)} disabled={busy}>
                {profiles.map((p) => (
                  <option key={p.name} value={p.name}>
                    {p.name} ({p.protocol === "sftp" ? "SFTP" : "GitHub Pages"})
                  </option>
                ))}
              </select>
            </label>

            {blockedNoSecret && (
              <div className="modal-error" style={{ color: "var(--danger, #c00)" }}>
                Kein Secret für „{selected}". Bitte im Einstellungen-Dialog setzen.
              </div>
            )}

            <div style={{ display: "flex", gap: "0.5rem", marginTop: "0.5rem" }}>
              <button type="button" onClick={dryRun} disabled={busy || !selected}>Dry-Run</button>
              <button type="button" onClick={deploy} disabled={busy || !selected || blockedNoSecret}>
                Deploy ausführen
              </button>
            </div>

            {diff && (
              <div style={{ marginTop: "0.6rem", border: "1px solid var(--border)", padding: "0.5rem", borderRadius: 4 }}>
                <div>
                  <strong>{diff.strategy === "incremental" ? "Inkrementell" : "Full-Upload"}</strong>
                  {diff.reason && <span className="muted small"> — {diff.reason}</span>}
                </div>
                <div className="muted small">
                  {diff.upload.length} Datei(en) · {fmtBytes(diff.upload_bytes)} ·{" "}
                  {diff.orphan_remote.length} verwaiste Remote-Datei(en)
                </div>
                {diff.upload.length > 0 && (
                  <details style={{ marginTop: "0.3rem" }}>
                    <summary>Upload-Liste</summary>
                    <ul style={{ margin: "0.3rem 0 0", paddingLeft: "1rem", fontSize: "0.85rem", maxHeight: 160, overflow: "auto" }}>
                      {diff.upload.map((f) => <li key={f}>{f}</li>)}
                    </ul>
                  </details>
                )}
                {diff.orphan_remote.length > 0 && (
                  <details>
                    <summary>Verwaiste Remote-Dateien (werden nicht gelöscht)</summary>
                    <ul style={{ margin: "0.3rem 0 0", paddingLeft: "1rem", fontSize: "0.85rem" }}>
                      {diff.orphan_remote.map((f) => <li key={f}>{f}</li>)}
                    </ul>
                  </details>
                )}
              </div>
            )}

            {log.length > 0 && (
              <div style={{ marginTop: "0.6rem", flex: 1, overflow: "auto", border: "1px solid var(--border)", padding: "0.4rem", borderRadius: 4, fontFamily: "monospace", fontSize: "0.8rem", background: "#fafafa" }}>
                {log.map((l, i) => (
                  <div key={i} style={{ color: l.level === "error" ? "var(--danger, #c00)" : undefined }}>
                    {l.text}
                  </div>
                ))}
              </div>
            )}

            {error && !log.some((l) => l.text.includes(error)) && (
              <div className="modal-error" style={{ color: "var(--danger, #c00)", marginTop: "0.4rem" }}>{error}</div>
            )}
          </>
        )}

        <div className="modal-actions">
          <button type="button" onClick={onClose} disabled={busy}>Schließen</button>
        </div>
      </div>
    </div>
  );
}
