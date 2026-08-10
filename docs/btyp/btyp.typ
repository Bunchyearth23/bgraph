// ── btyp — thème Typst de Bunchy ─────────────────────────────────────────────
// Un seul thème, deux corps : A4 (document) et Présentation (16:9 / 4:3),
// commutables par le booléen `presentation` de .with(...). Chaque primitive se
// plie au mode : slide() coule en sections en A4, devient une page pleine en
// présentation.
//
//   #show: bunchy-theme.with(presentation: true)   → deck 16:9
//   #show: bunchy-theme.with()                      → document A4
//   #show: bunchy-theme.with(presentation: true, paper: "presentation-4-3")
//   #show: bunchy-theme.with(paper: "us-letter")    → autre papier, mode doc

#let font-regular = "Archivo"
// Pile de repli explicite : seule la variante *Regular* de JetBrainsMono NFM est
// installée ici. Les tokens gras/italiques de la coloration syntaxique tombaient
// donc en silence sur DejaVu Sans Mono — visible dans `pdffonts`. Liberation Mono,
// qui a ses quatre variantes, reprend la main avant ce repli par défaut.
#let font-code = ("JetBrainsMono NFM", "JetBrainsMono NF", "Liberation Mono")

// ── Palette — jetons centralisés (changez `accent` pour reskinner tout) ───────
#let accent = rgb("#2563eb")
#let ink    = rgb("#1a1a1a")
#let muted  = rgb("#8a8a8a")
#let faint  = rgb("#f0f0f0")

// Jetons sémantiques — extraits de callout() pour être réutilisables ailleurs
// (badge, dtable, diagrammes Mermaid…). Sémantique stable dans tout le corpus :
// ok = ce qui tient · warn = la nuance, à mesurer · danger = ce qui casse.
#let ok-ink      = rgb("#1b8a3a")
#let ok-bg       = rgb("#ecf8ef")
#let warn-ink    = rgb("#c77700")
#let warn-bg     = rgb("#fff6e9")
#let danger-ink  = rgb("#c62828")
#let danger-bg   = rgb("#fdeded")
#let accent-bg   = rgb("#eef3ff")

// ── État interne — le mode de rendu, lu par toutes les primitives ─────────────
#let _mode  = state("btyp-pres", false)
#let _title = state("btyp-title", none)

// ── Le thème ──────────────────────────────────────────────────────────────────
#let bunchy-theme(body, presentation: false, paper: auto) = {
  let pres = presentation
  let fmt = if paper != auto { lower(paper) } else if pres { "presentation-16-9" } else { "a4" }

  set page(
    paper: fmt,
    margin: if pres { (x: 1.8cm, top: 1.5cm, bottom: 1.3cm) } else { (x: 1cm, y: 1cm) },
    footer: context {
      let is-pres = _mode.get()
      let pg = counter(page).display()
      let total = counter(page).final().first()
      if is-pres {
        set text(font: font-regular, size: 11pt, fill: muted)
        grid(
          columns: (1fr, auto),
          align(left + horizon)[#_title.get()],
          align(right + horizon)[#pg / #total],
        )
      } else {
        set text(font: font-regular, size: 9pt, fill: muted)
        align(center)[#pg / #total]
      }
    },
  )

  set text(font: font-regular, size: if pres { 19pt } else { 11pt }, fill: ink)
  set par(justify: not pres, leading: if pres { 0.75em } else { 0.6em })
  set list(spacing: if pres { 0.9em } else { 0.55em }, marker: text(fill: accent)[•])
  set enum(spacing: if pres { 0.9em } else { 0.55em })

  // Hiérarchie de titres
  set heading(numbering: none)
  show heading.where(level: 1): set text(size: if pres { 30pt } else { 17pt }, weight: 800, fill: ink)
  show heading.where(level: 2): set text(size: if pres { 24pt } else { 13pt }, weight: 700, fill: accent)
  show heading.where(level: 3): set text(size: if pres { 19pt } else { 11.5pt }, weight: 700, fill: ink)
  show heading: it => { it; v(if pres { 0.3em } else { 0.2em }) }

  // Légendes de figure
  show figure.caption: set text(size: if pres { 15pt } else { 9pt }, fill: muted)

  // Blocs de code (conservé, rendu adaptatif)
  show raw.where(block: true): it => {
    set text(font: font-code, size: if pres { 15pt } else { 9pt })
    block(
      fill: faint,
      inset: if pres { 12pt } else { 10pt },
      radius: 4pt,
      width: 100%,
      stroke: 0.5pt + muted,
      // Pas d'étiquette quand le bloc n'a pas de langue : un pseudo-code ou un
      // schéma d'algorithme n'est pas du « Language: none ».
      if it.lang == none { it } else {
        text(font: font-regular, size: if pres { 11pt } else { 8pt }, fill: muted)[_\/\/ Language: #it.lang _] + it
      },
    )
  }
  // Code en ligne — la police mono DOIT être posée ici aussi : sans elle, Typst
  // retombe sur son mono par défaut (DejaVu Sans Mono), qui n'a rien à voir avec
  // le reste du document et se trahit dans `pdffonts`. Le 0.92em compense la
  // chasse de JetBrains, plus large que celle d'Archivo à taille égale.
  show raw.where(block: false): it => box(
    fill: faint, inset: (x: 3pt, y: 0pt), outset: (y: 3pt), radius: 2pt,
    text(font: font-code, size: 0.92em, it),
  )

  _mode.update(pres)
  body
}

// ── line-sep — filet horizontal (conservé) ────────────────────────────────────
#let line-sep() = line(length: 100%, stroke: 0.5pt + muted)

// ── cover — page/slide de titre ───────────────────────────────────────────────
#let cover(title: none, subtitle: none, author: none, date: none) = {
  _title.update(title)
  context {
    if _mode.get() {
      page(header: none, footer: none)[
        #set align(center + horizon)
        #text(size: 40pt, weight: 800, fill: ink)[#title]
        #if subtitle != none { v(0.5em); text(size: 22pt, fill: muted)[#subtitle] }
        #v(1em)
        #line(length: 28%, stroke: 1.5pt + accent)
        #v(0.9em)
        #if author != none { text(size: 18pt)[#author]; linebreak() }
        #if date != none { text(size: 16pt, fill: muted)[#date] }
      ]
    } else {
      align(center)[
        #text(size: 26pt, weight: 800, fill: ink)[#title]
        #if subtitle != none { v(0.4em); text(size: 15pt, fill: muted)[#subtitle] }
        #v(0.5em)
        #line(length: 30%, stroke: 1pt + accent)
        #if author != none { v(0.4em); text(size: 12pt)[#author] }
        #if date != none { linebreak(); text(size: 11pt, fill: muted)[#date] }
      ]
      v(1em)
    }
  }
}

// ── section — intercalaire (page pleine en présentation, titre §1 en A4) ──────
#let section(name) = context {
  if _mode.get() {
    page(header: none, footer: none)[
      #set align(horizon)
      #block(inset: (left: 1cm))[
        #line(length: 3cm, stroke: 4pt + accent)
        #v(0.5em)
        #text(size: 40pt, weight: 800, fill: ink)[#name]
      ]
    ]
  } else {
    pagebreak(weak: true)
    heading(level: 1)[#name]
    line-sep()
    v(0.4em)
  }
}

// ── slide — l'unité de contenu (page en présentation, section qui coule en A4) ─
#let slide(body, title: none) = context {
  if _mode.get() {
    pagebreak(weak: true)
    if title != none { heading(level: 2)[#title] }
    body
  } else {
    if title != none { heading(level: 2)[#title] }
    body
  }
}

// ── callout — encadré (info / warn / danger / ok) ─────────────────────────────
#let callout(body, kind: "info", title: none) = {
  let palette = (
    info:   (accent,     accent-bg),
    warn:   (warn-ink,   warn-bg),
    danger: (danger-ink, danger-bg),
    ok:     (ok-ink,     ok-bg),
  )
  let c = palette.at(kind, default: palette.info)
  block(
    width: 100%,
    fill: c.at(1),
    stroke: (left: 3pt + c.at(0)),
    inset: 10pt,
    radius: 2pt,
    {
      if title != none { text(weight: 700, fill: c.at(0))[#title]; linebreak() }
      body
    },
  )
}

// ── two-col — deux colonnes (identique dans les deux modes) ───────────────────
#let two-col(l, r, gutter: 1.2em, columns: (1fr, 1fr)) = grid(
  columns: columns, column-gutter: gutter, l, r,
)

// ── stat / stats-row — tuiles de chiffres-clés (échelle avec le mode) ─────────
#let stat(value, label) = block(
  width: 100%, inset: 12pt, radius: 4pt, fill: faint, stroke: 0.5pt + muted,
  {
    set align(center)
    text(weight: 800, fill: accent, size: 1.8em)[#value]
    linebreak()
    text(fill: muted, size: 0.7em)[#label]
  },
)
#let stats-row(..items) = grid(
  columns: items.pos().len(),
  column-gutter: 1em, row-gutter: 1em,
  ..items.pos().map(it => stat(it.at(0), it.at(1))),
)

// ── badge — pastille inline de statut, de verdict ou de marquage ─────────────
// `#badge("prouvé", kind: "ok")` · kinds : info (défaut), ok, warn, danger, neutral.
// Pensée pour les documents qui distinguent le fait de l'hypothèse : la pastille
// se pose DANS le paragraphe, sans en casser le fil.
#let badge(label, kind: "info") = {
  let palette = (
    info:    (accent,     accent-bg),
    ok:      (ok-ink,     ok-bg),
    warn:    (warn-ink,   warn-bg),
    danger:  (danger-ink, danger-bg),
    neutral: (muted,      faint),
  )
  let c = palette.at(kind, default: palette.info)
  box(
    inset: (x: 4pt, y: 1.5pt), outset: (y: 2.5pt), radius: 3pt,
    fill: c.at(1), stroke: 0.5pt + c.at(0),
    text(size: 0.68em, weight: 700, fill: c.at(0))[#upper(label)],
  )
}

// ── dtable — tableau au style maison ─────────────────────────────────────────
// Filets horizontaux seulement, en-tête teinté, inset qui suit le mode.
//   #dtable(header: ([Champ], [Rôle]), [renewal], [le discriminant], …)
// `columns` vaut par défaut le nombre de colonnes de l'en-tête.
#let dtable(header: (), columns: auto, ..cells) = context {
  let pres = _mode.get()
  let cols = if columns == auto { header.len() } else { columns }
  set text(size: if pres { 0.82em } else { 0.95em })
  table(
    columns: cols,
    inset: if pres { 8pt } else { 5.5pt },
    align: left + top,
    fill: (x, y) => if y == 0 { faint } else { none },
    stroke: (x, y) => (
      top: if y <= 1 { 0.8pt + muted } else { 0.4pt + rgb("#dddddd") },
      bottom: 0.4pt + rgb("#dddddd"),
      left: 0pt, right: 0pt,
    ),
    ..header.map(h => text(weight: 700, fill: ink)[#h]),
    ..cells,
  )
}

// ── diagram — figure légendée pour un schéma déjà chargé ─────────────────────
// Prend un CONTENU (l'image), jamais un chemin : dans Typst, un chemin relatif se
// résout depuis le fichier où il est écrit — un `image("assets/…")` passé au thème
// serait cherché dans btyp/, pas chez l'appelant. Donc :
//   #diagram(image("assets/diagrams/x.svg", width: 96%), caption: [Légende.])
// Le style de légende, lui, vient du thème (show figure.caption).
#let diagram(img, caption: none) = figure(img, caption: caption)

// ── lead — chapeau : la phrase qui porte le document ─────────────────────────
#let lead(body) = block(
  width: 100%, inset: (left: 10pt), stroke: (left: 3pt + accent),
  text(size: 1.08em, fill: ink)[#body],
)
