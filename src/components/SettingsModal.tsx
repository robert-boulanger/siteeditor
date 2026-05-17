import { useState } from "react";
import { DeployProfilesTab } from "./DeployProfilesTab";
import { ProjectTab, type SiteSettings } from "./ProjectTab";

export type SettingsTab = "project" | "deploy";

type Props = {
  onClose: () => void;
  initialTab?: SettingsTab;
  onProjectSaved?: (s: SiteSettings) => void;
};

export function SettingsModal({ onClose, initialTab = "project", onProjectSaved }: Props) {
  const [tab, setTab] = useState<SettingsTab>(initialTab);
  const [busy, setBusy] = useState(false);
  const [projectDirty, setProjectDirty] = useState(false);
  const [deployDirty, setDeployDirty] = useState(false);

  function switchTab(next: SettingsTab) {
    if (next === tab) return;
    const leaving = tab === "project" ? projectDirty : deployDirty;
    if (leaving) {
      const ok = confirm("Ungespeicherte Änderungen verwerfen?");
      if (!ok) return;
      if (tab === "project") setProjectDirty(false);
      else setDeployDirty(false);
    }
    setTab(next);
  }

  function handleClose() {
    if (busy) return;
    if (projectDirty || deployDirty) {
      const ok = confirm("Ungespeicherte Änderungen verwerfen und schließen?");
      if (!ok) return;
    }
    onClose();
  }

  return (
    <div className="modal-backdrop" onClick={handleClose}>
      <div
        className="modal settings-modal"
        onClick={(e) => e.stopPropagation()}
        style={{ minWidth: 620, maxHeight: "85vh", display: "flex", flexDirection: "column" }}
      >
        <h3>Projekt-Einstellungen</h3>

        <div role="tablist" style={{ display: "flex", gap: "0.25rem", borderBottom: "1px solid var(--border)", marginBottom: "0.6rem" }}>
          <TabButton active={tab === "project"} dirty={projectDirty} onClick={() => switchTab("project")}>
            Projekt
          </TabButton>
          <TabButton active={tab === "deploy"} dirty={deployDirty} onClick={() => switchTab("deploy")}>
            Deploy-Profile
          </TabButton>
        </div>

        <div style={{ flex: 1, overflow: "auto" }}>
          {tab === "project" && (
            <ProjectTab
              busy={busy}
              setBusy={setBusy}
              onSaved={onProjectSaved}
              onDirtyChange={setProjectDirty}
            />
          )}
          {tab === "deploy" && (
            <DeployProfilesTab
              busy={busy}
              setBusy={setBusy}
              onDirtyChange={setDeployDirty}
            />
          )}
        </div>

        <div className="modal-actions">
          <button type="button" onClick={handleClose} disabled={busy}>Schließen</button>
        </div>
      </div>
    </div>
  );
}

function TabButton({
  active, dirty, onClick, children,
}: { active: boolean; dirty: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onClick}
      style={{
        background: "none",
        border: "none",
        borderBottom: active ? "2px solid var(--accent, #36c)" : "2px solid transparent",
        padding: "0.4rem 0.8rem",
        cursor: "pointer",
        fontWeight: active ? 600 : 400,
        marginBottom: -1,
      }}
    >
      {children}
      {dirty && <span style={{ marginLeft: 4, color: "var(--accent, #36c)" }}>•</span>}
    </button>
  );
}
