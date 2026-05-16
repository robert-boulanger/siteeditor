# Theme Authoring — Anleitung für AI-Sessions

Dieses Dokument ist der **Auftragsbeschrieb für eine Claude-Session, die
ein neues Theme für den siteeditor erstellen soll**. Es ist absichtlich
so geschrieben, dass eine fremde Session ohne Rückfragen ein lauffähiges
Theme produzieren kann.

> **Typischer Auftrag des Users:** „Schau dir Webseite XY an. Bau mir ein
> Theme für meinen siteeditor, das so ähnlich aussieht. Es muss auf
> Desktop, Tablet und Handy funktionieren."

## Vertrag (kurz)

- Ein Theme ist ein eigenständiges Verzeichnis unter `themes/<slug>/`.
- Pflichtdateien: `theme.json`, `styles/main.css`, `templates/page.html`,
  `templates/index.html`, `templates/404.html`, `templates/partials/head.html`,
  `templates/partials/menu.html`, `templates/partials/_menu_macros.html`.
- **Keine Inheritance.** Jedes Theme ist vollständig. Startpunkt:
  `themes/default/` kopieren.
- HTML-Struktur (Tag-Wahl, Block-Schleife, Variablen) **darf nicht
  geändert werden** — sonst rendert der Sitebuilder den Inhalt nicht
  mehr richtig.
- **CSS-Klassen** der Templates folgen einer festen BEM-Konvention
  (siehe unten). Diese Klassen sind die Public-API: ein Theme stylt sie,
  fügt aber keine weg.
- Aktiviert wird ein Theme durch `active_theme: "<slug>"` in `site.json`.
  Der User wählt das in der App via Sidebar-Dropdown.

## Workflow

1. **Inspirations-Site analysieren.** Hauptfarben, Schriftart-Familien,
   typografische Rhythmik (Headline-Größen, Zeilenhöhe, Abstände),
   Button-Stil, Bildbehandlung (Rundungen, Schatten), Layout-Container
   (max-width), Navigation (horizontal/vertical, sticky/scroll), Footer.
2. **`themes/default/` als Startpunkt vollständig kopieren** nach
   `themes/<slug>/`. Nichts löschen, keine Templates abkürzen.
3. **`theme.json` anpassen:**
   - `name`: muss gleich dem Verzeichnisnamen sein.
   - `display_name`, `version`, `author`, `description` setzen.
   - `css_variables`: Werte an die Inspirations-Site angleichen
     (Farben als Hex/HSL, Schrift-Stack mit Web-safe Fallbacks).
   - `supported_blocks` aus dem Default übernehmen, es sei denn das
     neue Theme rendert einen Blocktyp wirklich nicht — dann in
     `unsupported_blocks` verschieben und im Template das `{% elif %}`
     für diesen Typ entfernen.
4. **`styles/main.css` schreiben** — die ganze Optik lebt hier. Regeln
   ausschließlich gegen die in dieser Doc gelisteten BEM-Klassen.
5. **Templates nur dann anfassen,** wenn die Struktur fundamental
   anders sein muss (z.B. Sidebar-Layout statt Single-Column). Dann
   **alle in `theme_default_classes.rs` getesteten Klassen erhalten**.
6. **Lokal testen:**
   - `cargo test -p sitebuilder` muss grün bleiben.
   - In der App das Theme via Dropdown aktivieren, Build + Reload prüfen.

## Verzeichnisstruktur (Pflicht)

```
themes/<slug>/
├── theme.json
├── styles/
│   └── main.css
└── templates/
    ├── page.html
    ├── index.html         # in der Regel: {% extends "page.html" %}
    ├── 404.html
    └── partials/
        ├── head.html
        ├── menu.html
        └── _menu_macros.html
```

Zusätzliche Assets (Hintergrundbilder, Fonts, Icons) gehören unter
`themes/<slug>/assets/…` und werden über `/assets/...`-Pfade
referenziert (Sitebuilder kopiert den Ordner in den Build-Output).

## `theme.json` — Felder

```json
{
  "spec_version": "0.2",
  "name": "<slug>",
  "display_name": "Lesbarer Name",
  "version": "0.1.0",
  "author": "...",
  "description": "...",
  "supported_blocks": ["hero","text","image","gallery","video","cta","columns","quote"],
  "unsupported_blocks": [],
  "css_variables": {
    "--color-primary":   "#…",
    "--color-bg":        "#…",
    "--color-text":      "#…",
    "--font-body":       "…",
    "--font-heading":    "…",
    "--radius":          "…px",
    "--spacing-section": "…rem"
  }
}
```

Diese sieben CSS-Variablen sind **reserviert** (THEME_SPEC v0.2 §6). Sie
werden vom Sitebuilder in `:root` injiziert; eigene Variablen mit
`--<theme-slug>-*`-Prefix dürfen ergänzt werden.

## BEM-Konvention der Templates

Reservierte Klassen, gegen die ein Theme stylt. **Keine davon
entfernen — sonst brechen die Snapshot-Tests in `theme_default_classes.rs`.**
Reihenfolge: Block-Wrapper → Modifier → Elemente.

### Globales

| Klasse | Zweck |
|---|---|
| `.site-header`, `.site-title`, `.site-nav`, `.site-footer` | Site-Chrome (Header/Footer/Hauptnavigation) |
| `.page` | Container für den gerenderten Seiteninhalt |
| `.page--error`, `.page__error-code`, `.page__error-message`, `.page__error-link` | 404-Seite |
| `.is-active` | State-Klasse, z.B. auf `<li>` im Menü für die aktive Seite |
| `.block` | Pflicht-Wrapper auf jedem Top-Level-Block (siehe pro Blocktyp) |

### Blocktyp `hero`

Wrapper: `<section class="block hero hero--align-{left|center|right}">`

| Element | Klasse |
|---|---|
| Figure-Wrapper (optionales Bild) | `.hero__figure` |
| `<img>` | `.hero__image` |
| `<figcaption>` | `.hero__caption` |
| `<h1>` Headline | `.hero__headline` |
| `<p>` Sub-Headline | `.hero__sub` |

### Blocktyp `text` (prose)

Wrapper: `<section class="block prose prose--{default|lead|callout}">`
Inhalt ist gerendertes Markdown — keine inneren BEM-Klassen.

### Blocktyp `image`

Wrapper: `<figure class="block image image--{normal|wide|full|narrow}">`

| Element | Klasse |
|---|---|
| `<img>` | `.image__img` |
| `<figcaption>` | `.image__caption` |

### Blocktyp `gallery`

Wrapper: `<section class="block gallery gallery--{grid|...}" style="--gallery-cols: N">`

| Element | Klasse |
|---|---|
| Figure pro Bild | `.gallery__item` |
| `<img>` | `.gallery__image` |
| `<figcaption>` | `.gallery__caption` |

### Blocktyp `video`

Wrapper: `<figure class="block video">`

| Element | Klasse |
|---|---|
| `<video>` | `.video__player` |
| `<figcaption>` | `.video__caption` |

### Blocktyp `cta`

Wrapper: `<div class="block cta">` (nicht `<p>` — verhindert mehrere
Buttons nebeneinander).

| Element | Klasse |
|---|---|
| `<a>` Button | `.cta__btn .cta__btn--{primary\|secondary}` |

### Blocktyp `columns`

Wrapper: `<section class="block columns columns--{2|3}">`

| Element | Klasse |
|---|---|
| Eine Spalte | `.columns__col` |
| Innerer Block (Text/Image/CTA/Quote) | `.columns__item .columns__item--{text\|image\|cta\|quote}` zusätzlich zu den BEM-Klassen des inneren Blocktyps |

### Blocktyp `quote`

Wrapper: `<blockquote class="block quote">`

| Element | Klasse |
|---|---|
| `<p>` Zitattext | `.quote__text` |
| `<cite>` Quelle | `.quote__cite` |

## Navigation (Hamburger + hierarchisches Menü)

Pages können verschachtelt sein (`pages/about.md` + `pages/about/team.md` →
parent/child). Das Menü rendert diese Hierarchie **rekursiv** über ein
Tera-Macro — dafür gibt es die Pflichtdatei
`templates/partials/_menu_macros.html`. Ein Theme darf das CSS frei
gestalten, aber Markup-Struktur und Klassen sind Vertrag.

### `_menu_macros.html` (Pflicht, wörtlich übernehmen)

```jinja
{% macro menu_list(items, depth) %}
  <ul class="nav-list nav-list--depth-{{ depth }}">
    {% for item in items %}
      <li class="nav-item{% if item.slug == page.slug %} is-active{% endif %}{% if item.children %} has-children{% endif %}">
        <a class="nav-link" href="{{ item.url | safe }}">{{ item.title }}</a>
        {% if item.children %}
          {{ self::menu_list(items=item.children, depth=depth + 1) }}
        {% endif %}
      </li>
    {% endfor %}
  </ul>
{% endmacro menu_list %}
```

Rekursion über `self::menu_list(...)` — kein Tiefen-Limit, alle Levels.

### `menu.html` (Hamburger-Pattern, ohne JS)

```jinja
{% import "partials/_menu_macros.html" as nav %}
<input type="checkbox" id="nav-toggle" class="nav-toggle" aria-hidden="true">
<label for="nav-toggle" class="nav-burger" aria-label="Menü öffnen/schließen">
  <span></span><span></span><span></span>
</label>
<nav class="site-nav" aria-label="Hauptnavigation">
  {{ nav::menu_list(items=site.menu, depth=0) }}
</nav>
```

Der Checkbox-Hack (`#nav-toggle:checked ~ .site-nav { … }`) öffnet/
schließt das Menü auf Mobile ohne JavaScript.

### Reservierte Klassen (Public-API)

| Klasse | Zweck |
|---|---|
| `.nav-toggle` | versteckte Checkbox, steuert Open-State (Mobile) |
| `.nav-burger` | sichtbares `<label>` mit den drei Strichen |
| `.site-nav` | Wrapper-`<nav>` um die Top-Level-Liste |
| `.nav-list` | jede `<ul>` der Hierarchie |
| `.nav-list--depth-N` | Tiefen-Modifier (`0` = Top-Level, `1+` = Submenüs) |
| `.nav-item` | jedes `<li>` |
| `.nav-link` | jedes `<a>` |
| `.has-children` | `<li>`, das ein Submenü öffnet (Dropdown-Hook) |
| `.is-active` | `<li>` der aktiven Seite |

### Empfohlene CSS-Patterns

- **Mobile (≤ Tablet-Breakpoint):** `.site-nav` standardmäßig
  ausgeblendet (`display: none` oder `max-height: 0`), durch
  `.nav-toggle:checked ~ .site-nav { display: block }` öffnen.
  `.nav-burger` nur hier sichtbar.
- **Desktop:** `.nav-burger` und `.nav-toggle` ausblenden, `.site-nav`
  immer sichtbar. Top-Level `.nav-list--depth-0` horizontal flexen.
- **Dropdowns auf Desktop:** Subnav (`.nav-list--depth-1+`) absolut
  positioniert unter `.has-children`. Öffnen über `:hover` **und**
  `:focus-within` (Tastatur-Zugänglichkeit). Zwischen Trigger-Link und
  Dropdown **keinen Gap lassen**, sonst schließt das Hover beim
  Übergang — entweder mit `padding-bottom` am `<li>` oder mit einem
  unsichtbaren `::before` überbrücken.
- `is-active` bekommt einen sichtbaren State (Farbe, Underline) — auch
  in Submenüs.

## Responsive

**Mobile-First Pflicht.** Basis-Regeln gelten für Handy, Media-Queries
schalten Tablet/Desktop frei. Empfohlene Breakpoints (frei wählbar,
müssen aber alle drei Größen abdecken):

```css
/* Tablet  ≥ 640 px */
@media (min-width: 40em) { ... }

/* Desktop ≥ 1024 px */
@media (min-width: 64em) { ... }
```

Mindest-Erwartung an jedes Theme:

- **Handy (≤ 639 px)**: Single-Column-Layout, Header vertikal,
  Galerie 1-spaltig, kein horizontales Scrollen.
- **Tablet (640–1023 px)**: Header horizontal, mehrspaltige Galerien,
  `columns--2/--3` aktiv.
- **Desktop (≥ 1024 px)**: Headline-Skalierung, optional größere
  Abstände, max-width-Container.

Bilder und Videos haben global `max-width: 100%; height: auto`. Custom
Breakpoints für einzelne Blöcke sind erlaubt.

## Build & Tests

```bash
# Snapshot-Vertrag (muss grün bleiben — testet `default`-Theme)
cargo test -p sitebuilder

# Interaktiv im Editor
npm run tauri dev
# → Theme via Sidebar-Dropdown aktivieren
# → Speichern oder Build-Button → SSE lädt Preview-Tab neu
```

## Was du NICHT tun darfst

- Block-Schleife in `page.html` umstrukturieren, sodass Blocks nicht
  mehr in der `blocks`-Liste iteriert werden.
- BEM-Klassen aus den Templates entfernen oder umbenennen — bricht
  abgeleitete Themes und die Snapshot-Tests.
- Reservierte CSS-Variablen (`--color-primary` etc.) umbenennen.
- `extends`-Mechanik bauen (bewusst verworfen, siehe Phase 09
  Decision-Doc).
- JavaScript einbauen, ohne dass es der User explizit beauftragt
  (Themes sind statisch — kein JS-Bundle, keine Tracker, kein
  CDN-Bezug ohne Rückfrage).

## Ablage des fertigen Themes

Im Workspace-Root unter `themes/<slug>/`. Danach in `site.json` (eines
konkreten Projekts):

```json
{ "active_theme": "<slug>", ... }
```

Oder in der App via Sidebar-Dropdown auswählen — Build + Reload laufen
automatisch.
