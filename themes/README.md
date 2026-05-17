# Themes für siteeditor — Anleitung für LLM-Assistenten

Diese Datei liegt absichtlich im `themes/`-Ordner jedes neuen Projekts.
Wenn dich der Nutzer bittet, **ein neues Theme zu erzeugen** oder ein
bestehendes **anzupassen**, lies sie zuerst durch — sie definiert den
verbindlichen Theme-Kontrakt (Spec 0.2). Themes, die den Kontrakt
verletzen, werden vom Builder mit einem Validation-Report abgelehnt
und nicht ausgespielt.

---

## 1. Ordnerstruktur (Pflicht)

Ein Theme ist ein Unterordner unter `themes/<name>/`. Der Name ist
zugleich der Slug (`a-z`, `0-9`, `-`, `_`) — er erscheint im
`site.json` und darf nach dem Anlegen nicht mehr umbenannt werden,
ohne den Verweis dort mitzuziehen.

```
themes/<name>/
├── theme.json                          # Manifest (Pflicht)
├── styles/
│   └── main.css                        # Einstiegs-CSS (Pflicht)
├── templates/
│   ├── page.html                       # Pflicht
│   ├── index.html                      # Pflicht
│   ├── 404.html                        # Pflicht
│   └── partials/
│       ├── head.html                   # Pflicht
│       └── menu.html                   # Pflicht
└── assets/                             # Optional (Fonts, Bilder, …)
```

Alle fünf Pflicht-Templates **müssen** existieren — auch wenn sie nur
`{% include %}`s enthalten. Fehlt eines, schlägt die Validierung mit
`TEMPLATE_MISSING` fehl.

---

## 2. `theme.json` — Manifest

Minimal-Beispiel:

```json
{
  "spec_version": "0.2",
  "name": "mein-theme",
  "display_name": "Mein Theme",
  "version": "0.1.0",
  "author": "<dein Name>",
  "description": "Ein-Satz-Beschreibung.",
  "supported_blocks": [
    "hero", "text", "image", "gallery", "video", "cta", "columns", "quote"
  ],
  "unsupported_blocks": [],
  "css_variables": {
    "--color-primary":   "#000000",
    "--color-bg":        "#ffffff",
    "--color-text":      "#111111",
    "--font-body":       "system-ui, sans-serif",
    "--font-heading":    "Georgia, serif",
    "--radius":          "4px",
    "--spacing-section": "5rem"
  }
}
```

### Pflichtfelder
- `spec_version` — derzeit `"0.2"`. Andere Werte werden abgelehnt.
- `name` — identisch zum Ordnernamen.
- `version` — semver (`MAJOR.MINOR.PATCH`).
- `supported_blocks` — Liste der Block-Typen, die das Theme rendert.
  Pflicht-Set: `hero, text, image, gallery, video, cta, columns,
  quote`. Du darfst Blocks weglassen, indem du sie in
  `unsupported_blocks` listest **und** aus `supported_blocks`
  entfernst — der Editor blendet sie dann in der Block-Palette aus.
- `css_variables` — alle sieben Pflicht-Variablen unten **müssen**
  gesetzt sein, auch wenn der Wert leer ist.

### Pflicht-CSS-Variablen
```
--color-primary
--color-bg
--color-text
--font-body
--font-heading
--radius
--spacing-section
```
Diese sieben Variablen sind der Schnittpunkt zwischen Theme und
Sitebuilder: der Block-Editor zeigt sie als Theme-Tokens an, und
generierte Inline-Styles greifen darauf zurück. Weitere Custom-Vars
sind erlaubt und beginnen ebenfalls mit `--`.

---

## 3. Templates — Tera-Subset

Die Templates werden von **Tera** (Rust-Port von Jinja2) gerendert.
Erlaubt ist das übliche Subset: `{{ … }}` für Ausgaben,
`{% if/for/include/block/extends %}` für Steuerung,
`{% macro/import %}` für Wiederverwendung.

**Verboten / vermeiden** (Theme-Sicherheit + Portabilität):
- `{% set_global %}` und Side-Effects auf den Site-Kontext
- Schreibende Filter wie `json_encode(...) | safe` für User-Daten
  ohne Eskapierung
- Inline-`<script>` mit dynamischen Server-/User-Inhalten ohne
  Escaping

### Verfügbarer Kontext

Für **alle** Templates:

| Variable    | Inhalt |
|---|---|
| `site`      | `site.json` — `title`, `base_url`, `nav`, … |
| `theme`     | Geparstes Manifest — `name`, `display_name`, `css_variables`, … |
| `pages`     | Liste aller Seiten (für Menüs) |
| `current`   | Aktuelle Seite mit `slug`, `title`, `path`, `frontmatter` |

Für `page.html` zusätzlich:

| Variable    | Inhalt |
|---|---|
| `content`   | Gerendertes HTML der Seite (aus Blocks + Markdown) |
| `blocks`    | Roh-Blockliste (selten gebraucht — `content` reicht) |

Für `index.html`:

| Variable    | Inhalt |
|---|---|
| `content`   | Wie bei `page.html`, für die Startseite |

Für `404.html`: nur `site` + `theme` + `pages`.

### Pflicht-Partials einbinden

`templates/page.html` und `templates/index.html` **müssen** den Head
einbinden und ein Menü zeigen:

```jinja
<!doctype html>
<html lang="{{ site.lang | default(value='de') }}">
<head>
  {% include "partials/head.html" %}
</head>
<body>
  {% include "partials/menu.html" %}
  <main>{{ content | safe }}</main>
</body>
</html>
```

`partials/head.html` ist verantwortlich für `<title>`, Meta-Tags,
Favicon und das CSS-Einbinden (`<link rel="stylesheet"
href="/styles/main.css">`). `partials/menu.html` baut die Navigation
aus `pages` + `site.nav`.

---

## 4. `styles/main.css`

Lege die Pflicht-CSS-Variablen unter `:root` an und benutze sie im
ganzen Stylesheet. **Keine externen Requests** (kein Google Fonts
CDN, keine Tracker, keine externen Bilder) — alles, was geladen wird,
muss unter `assets/` liegen.

```css
:root {
  --color-primary:   #000;
  --color-bg:        #fff;
  --color-text:      #111;
  --font-body:       system-ui, sans-serif;
  --font-heading:    Georgia, serif;
  --radius:          4px;
  --spacing-section: 5rem;
}

body { background: var(--color-bg); color: var(--color-text);
       font-family: var(--font-body); }
```

---

## 5. Block-Renderer

Jeder Block-Typ aus `supported_blocks` braucht ein konsistentes
HTML-Ergebnis im gerenderten `content`. Die Block-Render-Layer
generiert das HTML; du gestaltest es per CSS-Klassen:

| Block      | Wrapper-Klasse |
|---|---|
| `hero`     | `.block-hero` |
| `text`     | `.block-text` |
| `image`    | `.block-image` |
| `gallery`  | `.block-gallery` |
| `video`    | `.block-video` |
| `cta`      | `.block-cta` |
| `columns`  | `.block-columns` (mit `.col` Kindern) |
| `quote`    | `.block-quote` |

Style alle, die du in `supported_blocks` listest. Listest du einen
Block dort, ohne ihn zu stylen, sieht der Nutzer beim Auswählen ein
ungestyltes Element — schlechter UX-Default.

---

## 6. Was du **nicht** tun sollst

- **Kein Build-Tool im Theme.** Keine `package.json`, kein Tailwind-
  CLI, kein PostCSS-Pipeline. CSS wird 1:1 ausgespielt.
- **Keine externen Requests.** Fonts lokal unter `assets/fonts/`
  einbinden, nicht von Google Fonts ziehen.
- **Keine JavaScript-Frameworks.** Vanilla-JS in `assets/` ist ok,
  React/Vue/Svelte gehören nicht in ein Theme.
- **Keine globale State-Mutation in Templates.** Tera-Templates sind
  reine Funktionen vom Kontext aufs HTML.
- **Manifest-Felder nicht erfinden.** Felder, die nicht im Kontrakt
  stehen, werden ignoriert — verlass dich nicht drauf.
- **Spec-Version nicht ändern.** Spec 0.2 ist Pflicht; ein Theme mit
  `"spec_version": "0.3"` wird abgelehnt.

---

## 7. Workflow für ein neues Theme

1. Ordner `themes/<slug>/` anlegen.
2. `theme.json` mit allen Pflichtfeldern und Pflicht-Vars.
3. Alle fünf Pflicht-Templates anlegen (auch wenn sie nur
   `{% include %}`s enthalten).
4. `styles/main.css` mit `:root`-Block für die Pflicht-Vars.
5. Block-Klassen `.block-*` stylen für alle in
   `supported_blocks` gelisteten Typen.
6. Lokal builden (`Build` im App-Menü) — Validation-Report prüfen.

Wenn du ein bestehendes Theme **anpasst**, ändere nur das, worum der
Nutzer dich bittet. Versionsfeld in `theme.json` hochzählen (Patch
für Bugs, Minor für neue Variablen, Major für Brüche).

---

## 8. Referenz-Theme

`themes/default/` ist das Referenz-Theme. Studiere es, wenn etwas
unklar ist — es nutzt jedes Pflicht-Feature und bindet die
Inter-Schriftart lokal unter `assets/fonts/` ein.
