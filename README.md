# siteeditor

Ein schlanker Desktop-Editor für **statische Websites**. Inhalte werden als
Markdown mit YAML-Frontmatter gepflegt, das Layout kommt aus austauschbaren
**Themes**, der Build erzeugt reines HTML/CSS — kein CMS, kein Server, kein
JavaScript-Framework im Output.

Wofür gedacht:

- Persönliche Sites, kleine Firmen-Auftritte, Projekt-/Doku-Seiten
- Inhalte WYSIWYG-pflegen, ohne dafür einen Server, eine Datenbank oder
  einen Build-Pipeline-Account zu brauchen
- Auf SFTP-Hosting deployen (mit Dry-Run, Diff-Upload, Manifest-getriebener
  Inkrementellsynchronisation)

Technisch: **Tauri 2** (Rust-Backend) + **React 19** im Frontend, **Vite** als
Build-Tool. Der Sitebuilder ist eine eigenständige Rust-Crate, das gleiche gilt
für Projekt-IO, Deploy-Vertrag, SFTP-Adapter und Theme-Vertrag.

## Status

In aktiver Entwicklung. Kern (Pages, Blocks, Themes, Build, Preview, SFTP-Deploy,
Projekt-/Deploy-Einstellungen) läuft. Tests in jeder Crate parallel zum Code.

---

## Build & Entwicklung

Voraussetzung auf beiden Plattformen:

- **Rust** stable (≥ 1.78) via [rustup](https://rustup.rs)
- **Node.js** ≥ 20 (oder ein aktuelles LTS) mit npm
- Tauri-CLI: kommt als devDependency mit, kein globales Install nötig

Repo auschecken und einmalig:

```bash
npm install
```

Dev-Modus (Vite-HMR + Tauri-Hot-Reload):

```bash
npm run tauri dev
```

Release-Build (erzeugt das native Bundle für die aktuelle Plattform unter
`src-tauri/target/release/bundle/`):

```bash
npm run tauri build
```

Reine Tests:

```bash
cargo test                # alle Rust-Crates
npx tsc --noEmit          # TypeScript-Typecheck
```

### macOS

Benötigt **Xcode Command Line Tools** für den C-Linker und die System-SDKs:

```bash
xcode-select --install
```

Das reicht. Tauri findet WebKit über das System (kein Extra-Install). Beim
ersten `tauri build` legt macOS einen `.app`-Bundle (und optional `.dmg`) an;
für signierte Distribution braucht es ein Apple-Developer-Zertifikat —
Entwicklung lokal funktioniert ohne.

Apple-Silicon: alles native arm64, keine Rosetta-Tricks.

### Windows

Benötigt:

1. **MSVC-Buildtools** — entweder Visual Studio 2022 Community mit Workload
   *"Desktop development with C++"* oder die schlankeren
   [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   (Komponente *MSVC v143 — VS 2022 C++ x64/x86 build tools* + *Windows
   11 SDK*).
2. **WebView2-Runtime** — auf Windows 11 vorinstalliert, auf Windows 10 ggf.
   nachinstallieren ([Evergreen-Installer](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)).
3. **Rust** mit dem MSVC-Toolchain (Default beim `rustup` unter Windows):
   `rustup default stable-x86_64-pc-windows-msvc`.

Anschließend wie oben — `npm install` und `npm run tauri dev` / `tauri build`.
Der Release-Build erzeugt `.msi` und `.exe`-Installer.

### Hinweise zu Crates mit nativen Abhängigkeiten

- `keyring` nutzt auf macOS die System-Keychain, auf Windows den Credential
  Manager — beide Backends sind über Cargo-Features fest verdrahtet und
  brauchen keine Zusatz-Installation.
- `ssh2` (über den SFTP-Adapter) bringt libssh2 als Source mit und wird statisch
  gebaut — kein OpenSSL-System-Dep auf macOS, keine OpenSSL-Pain auf Windows.

---

## Projektstruktur

```
crates/
  projectfs/          Site-Projekt lesen/schreiben (site.json, pages/, assets/)
  theme-contract/     Theme-Manifest + Template-Konventionen
  sitebuilder/        Rendert pages/ + themes/ → .siteeditor/build/
  deploy-contract/    Trait Uploader + DiffStrategy
  deploy-sftp/        SFTP-Adapter (libssh2)
src-tauri/            Tauri-App: Commands, Menü, Preview-Server
src/                  React-Frontend (TipTap-WYSIWYG, Page-Tree, Settings…)
themes/default/       Mitgeliefertes Default-Theme
```

Projektdokumentation (Plan, Decisions, Bug-Reports pro Phase) lebt im
separaten `projectdocs/`-Tree außerhalb des Source-Baums.
