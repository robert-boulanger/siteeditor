/** Gemeinsame Frontend-Typen für die Deployment-Modals.
 *  Spiegelt das Schema aus `deploy-contract::profile` und der
 *  Tauri-Command-DTOs in `src-tauri/src/deploy_commands.rs`.
 */

export type Protocol = "sftp" | "ftp" | "github_pages";

export type AuthMethod =
  | { kind: "password"; user: string }
  | { kind: "ssh_key"; user: string; private_key_path: string }
  | { kind: "github_token"; user: string };

export type DeployProfile = {
  name: string;
  protocol: Protocol;
  host: string;
  port: number;
  auth: AuthMethod;
  remote_path: string;
  branch?: string | null;
  prefer_diff?: boolean;
};

export type DiffReportDto = {
  strategy: "incremental" | "full";
  reason?: string | null;
  upload: string[];
  orphan_remote: string[];
  upload_bytes: number;
};

export type ProgressPayload =
  | { kind: "connected" }
  | { kind: "diff_resolved"; upload_count: number; upload_bytes: number }
  | { kind: "file_start"; rel_path: string; size: number }
  | { kind: "file_done"; rel_path: string }
  | { kind: "manifest_written" }
  | { kind: "done"; uploaded: number; total_bytes: number }
  | { kind: "error"; message: string };

export const PROGRESS_EVENT = "deploy://progress";
