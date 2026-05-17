import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AuthMethod, DeployProfile, Protocol } from "./deployTypes";

type Props = {
  busy: boolean;
  setBusy: (b: boolean) => void;
  onDirtyChange?: (dirty: boolean) => void;
};

const SLUG_RE = /^[a-zA-Z0-9_-]+$/;

function emptyProfile(): DeployProfile {
  return {
    name: "",
    protocol: "sftp",
    host: "",
    port: 22,
    auth: { kind: "password", user: "" },
    remote_path: "",
    branch: null,
    prefer_diff: true,
  };
}

function applyProtocolDefaults(p: DeployProfile, protocol: Protocol): DeployProfile {
  if (protocol === "sftp") {
    return {
      ...p,
      protocol,
      port: p.port === 443 || p.port === 21 ? 22 : p.port,
      auth: p.auth.kind === "github_token" ? { kind: "password", user: "" } : p.auth,
      branch: null,
    };
  }
  if (protocol === "ftp") {
    const user =
      p.auth.kind === "password" || p.auth.kind === "ssh_key" ? p.auth.user : "";
    return {
      ...p,
      protocol,
      port: p.port === 22 || p.port === 443 ? 21 : p.port,
      auth: { kind: "password", user },
      branch: null,
    };
  }
  return {
    ...p,
    protocol,
    port: 443,
    auth: { kind: "github_token", user: p.auth.kind === "github_token" ? p.auth.user : "" },
    host: p.host || "github.com",
    branch: p.branch ?? "gh-pages",
  };
}

function protocolLabel(p: Protocol): string {
  switch (p) {
    case "sftp": return "SFTP";
    case "ftp": return "FTP";
    case "github_pages": return "GitHub Pages";
  }
}

export function DeployProfilesTab({ busy, setBusy, onDirtyChange }: Props) {
  const [profiles, setProfiles] = useState<DeployProfile[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [draft, setDraft] = useState<DeployProfile | null>(null);
  const [secret, setSecret] = useState("");
  const [hasSecret, setHasSecret] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);

  function markDirty() {
    if (!dirty) {
      setDirty(true);
      onDirtyChange?.(true);
    }
  }
  function clearDirty() {
    setDirty(false);
    onDirtyChange?.(false);
  }

  async function reload(selectName?: string | null) {
    const list = await invoke<DeployProfile[]>("deploy_list_profiles");
    setProfiles(list);
    if (selectName) {
      const p = list.find((p) => p.name === selectName) ?? null;
      setSelected(p?.name ?? null);
      setDraft(p ? structuredClone(p) : null);
      setSecret("");
      if (p) {
        try {
          const has = await invoke<boolean>("deploy_has_secret", { name: p.name });
          setHasSecret(has);
        } catch { setHasSecret(false); }
      } else setHasSecret(false);
    }
    clearDirty();
  }

  useEffect(() => { reload(null).catch((e) => setError(String(e))); }, []);

  function selectProfile(name: string) {
    const p = profiles.find((x) => x.name === name);
    if (!p) return;
    setSelected(name);
    setDraft(structuredClone(p));
    setSecret("");
    setError(null);
    clearDirty();
    invoke<boolean>("deploy_has_secret", { name }).then(setHasSecret).catch(() => setHasSecret(false));
  }

  function startNew() {
    setSelected(null);
    setDraft(emptyProfile());
    setSecret("");
    setHasSecret(false);
    setError(null);
    markDirty();
  }

  function updateDraft<K extends keyof DeployProfile>(k: K, v: DeployProfile[K]) {
    setDraft((d) => (d ? { ...d, [k]: v } : d));
    markDirty();
  }

  function updateAuth(next: AuthMethod) {
    setDraft((d) => (d ? { ...d, auth: next } : d));
    markDirty();
  }

  async function save() {
    if (!draft) return;
    setError(null);
    if (!SLUG_RE.test(draft.name)) {
      setError("Name: nur A-Z, a-z, 0-9, _ und -");
      return;
    }
    if (!draft.host.trim()) { setError("Host darf nicht leer sein"); return; }
    if (!draft.remote_path.trim()) { setError("Remote-Pfad darf nicht leer sein"); return; }
    setBusy(true);
    try {
      await invoke("deploy_save_profile", {
        profile: draft,
        secret: secret.length > 0 ? secret : null,
      });
      await reload(draft.name);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!selected) return;
    if (!confirm(`Profil "${selected}" inklusive Secret löschen?`)) return;
    setBusy(true);
    try {
      await invoke("deploy_delete_profile", { name: selected });
      setSelected(null);
      setDraft(null);
      await reload(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="settings-tab" style={{ display: "flex", flexDirection: "column", gap: "0.5rem" }}>
      <div style={{ display: "flex", gap: "1rem", alignItems: "flex-start" }}>
        <div style={{ width: 180, borderRight: "1px solid var(--border)", paddingRight: "0.5rem" }}>
          <div style={{ display: "flex", gap: "0.3rem", marginBottom: "0.4rem" }}>
            <button type="button" onClick={startNew} disabled={busy}>+ Neu</button>
            <button type="button" onClick={remove} disabled={busy || !selected}>Löschen</button>
          </div>
          {profiles.length === 0 && <p className="muted small">Keine Profile.</p>}
          <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
            {profiles.map((p) => (
              <li key={p.name}>
                <button
                  type="button"
                  onClick={() => selectProfile(p.name)}
                  style={{
                    width: "100%", textAlign: "left", padding: "0.3rem 0.4rem",
                    background: selected === p.name ? "var(--accent-soft, #eef)" : "transparent",
                    border: "none", cursor: "pointer", borderRadius: 3,
                  }}
                >
                  <strong>{p.name}</strong>
                  <div className="muted small">{protocolLabel(p.protocol)}</div>
                </button>
              </li>
            ))}
          </ul>
        </div>

        <div style={{ flex: 1, display: "flex", flexDirection: "column", gap: "0.5rem" }}>
          {!draft ? (
            <p className="muted">Profil links wählen oder <em>Neu</em>.</p>
          ) : (
            <>
              <label>
                <span>Name (Slug)</span>
                <input value={draft.name} onChange={(e) => updateDraft("name", e.currentTarget.value)} disabled={!!selected} />
              </label>
              <label>
                <span>Protokoll</span>
                <select
                  value={draft.protocol}
                  onChange={(e) => { setDraft(applyProtocolDefaults(draft, e.currentTarget.value as Protocol)); markDirty(); }}
                >
                  <option value="sftp">SFTP</option>
                  <option value="ftp">FTP</option>
                  <option value="github_pages">GitHub Pages</option>
                </select>
              </label>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 100px", gap: "0.5rem" }}>
                <label>
                  <span>Host</span>
                  <input value={draft.host} onChange={(e) => updateDraft("host", e.currentTarget.value)} />
                </label>
                <label>
                  <span>Port</span>
                  <input type="number" value={draft.port} onChange={(e) => updateDraft("port", Number(e.currentTarget.value))} />
                </label>
              </div>

              {draft.protocol === "sftp" && (
                <>
                  <label>
                    <span>Auth</span>
                    <select
                      value={draft.auth.kind}
                      onChange={(e) => {
                        const kind = e.currentTarget.value as "password" | "ssh_key";
                        updateAuth(kind === "password"
                          ? { kind: "password", user: draft.auth.kind === "password" ? draft.auth.user : "" }
                          : { kind: "ssh_key", user: "user" in draft.auth ? draft.auth.user : "", private_key_path: "" });
                      }}
                    >
                      <option value="password">Passwort</option>
                      <option value="ssh_key">SSH-Key</option>
                    </select>
                  </label>
                  <label>
                    <span>SSH-User</span>
                    <input
                      value={"user" in draft.auth ? draft.auth.user : ""}
                      onChange={(e) => {
                        if (draft.auth.kind === "password") updateAuth({ ...draft.auth, user: e.currentTarget.value });
                        else if (draft.auth.kind === "ssh_key") updateAuth({ ...draft.auth, user: e.currentTarget.value });
                      }}
                    />
                  </label>
                  {draft.auth.kind === "ssh_key" && (
                    <label>
                      <span>Private-Key-Pfad</span>
                      <input
                        value={draft.auth.private_key_path}
                        onChange={(e) => updateAuth({ ...draft.auth, private_key_path: e.currentTarget.value } as AuthMethod)}
                        placeholder="/Users/du/.ssh/id_ed25519"
                      />
                    </label>
                  )}
                  <label>
                    <span>Remote-Pfad (absolut)</span>
                    <input value={draft.remote_path} onChange={(e) => updateDraft("remote_path", e.currentTarget.value)} placeholder="/var/www/site" />
                  </label>
                </>
              )}

              {draft.protocol === "ftp" && (
                <>
                  <label>
                    <span>FTP-User</span>
                    <input
                      value={draft.auth.kind === "password" ? draft.auth.user : ""}
                      onChange={(e) => updateAuth({ kind: "password", user: e.currentTarget.value })}
                    />
                  </label>
                  <label>
                    <span>Remote-Pfad (absolut)</span>
                    <input value={draft.remote_path} onChange={(e) => updateDraft("remote_path", e.currentTarget.value)} placeholder="/htdocs" />
                  </label>
                </>
              )}

              {draft.protocol === "github_pages" && (
                <>
                  <label>
                    <span>GitHub-User</span>
                    <input
                      value={draft.auth.kind === "github_token" ? draft.auth.user : ""}
                      onChange={(e) => updateAuth({ kind: "github_token", user: e.currentTarget.value })}
                    />
                  </label>
                  <label>
                    <span>Repository (owner/repo)</span>
                    <input value={draft.remote_path} onChange={(e) => updateDraft("remote_path", e.currentTarget.value)} placeholder="octocat/mysite" />
                  </label>
                  <label>
                    <span>Branch</span>
                    <input value={draft.branch ?? ""} onChange={(e) => updateDraft("branch", e.currentTarget.value || null)} placeholder="gh-pages" />
                  </label>
                </>
              )}

              <label className="row">
                <input type="checkbox" checked={draft.prefer_diff !== false} onChange={(e) => updateDraft("prefer_diff", e.currentTarget.checked)} />
                <span>Diff-Upload bevorzugen (Fallback Full)</span>
              </label>

              <fieldset style={{ border: "1px dashed var(--border)", padding: "0.5rem", borderRadius: 4 }}>
                <legend>
                  Secret {hasSecret ? <span className="muted small">— im Keystore hinterlegt</span> : <span className="muted small">— noch nicht gesetzt</span>}
                </legend>
                <input
                  type="password"
                  value={secret}
                  onChange={(e) => { setSecret(e.currentTarget.value); markDirty(); }}
                  placeholder={draft.auth.kind === "github_token" ? "GitHub PAT" : draft.auth.kind === "ssh_key" ? "Key-Passphrase (optional)" : "Passwort"}
                  style={{ width: "100%" }}
                />
                <div className="muted small">Wird beim Speichern verschlüsselt im OS-Keystore abgelegt. Leer lassen, um den bestehenden Eintrag zu behalten.</div>
              </fieldset>
            </>
          )}

          {error && <div className="modal-error" style={{ color: "var(--danger, #c00)" }}>{error}</div>}
        </div>
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <button type="button" onClick={save} disabled={busy || !draft}>Profil speichern</button>
      </div>
    </div>
  );
}
