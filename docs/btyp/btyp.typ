// btyp — Bunchy's Typst theme
// One theme, two formats: A4 documents and 16:9 / 4:3 presentations.
// The presentation argument to .with(...) selects the mode. Each primitive
// adapts to it: slide() becomes a section in A4 and a full page
// in presentation mode.
//
//   #show: bunchy-theme.with(presentation: true)   → deck 16:9
//   #show: bunchy-theme.with()                      -> A4 document
//   #show: bunchy-theme.with(presentation: true, paper: "presentation-4-3")
//   #show: bunchy-theme.with(paper: "us-letter")    -> document on another paper size

#let font-regular = "Archivo"
// Explicit font fallback: only the Regular JetBrainsMono NFM face is
// installed here. Bold/italic syntax tokens otherwise silently fell back
// to DejaVu Sans Mono, as pdffonts revealed. Liberation Mono provides all
// four variants and is tried before that default fallback.
#let font-code = ("JetBrainsMono NFM", "JetBrainsMono NF", "Liberation Mono")

// Palette — centralized tokens; change accent to reskin the theme
#let accent = rgb("#2563eb")
#let ink    = rgb("#1a1a1a")
#let muted  = rgb("#8a8a8a")
#let faint  = rgb("#f0f0f0")

// Semantic tokens extracted from callout() for reuse by badges, tables
// and diagrams. Their meaning is consistent throughout the documents:
// ok = established; warn = qualification or measurement; danger = failure.
#let ok-ink      = rgb("#1b8a3a")
#let ok-bg       = rgb("#ecf8ef")
#let warn-ink    = rgb("#c77700")
#let warn-bg     = rgb("#fff6e9")
#let danger-ink  = rgb("#c62828")
#let danger-bg   = rgb("#fdeded")
#let accent-bg   = rgb("#eef3ff")

// Internal rendering mode, read by every primitive
#let _mode  = state("btyp-pres", false)
#let _title = state("btyp-title", none)

// Theme
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

  // Heading hierarchy
  set heading(numbering: none)
  show heading.where(level: 1): set text(size: if pres { 30pt } else { 17pt }, weight: 800, fill: ink)
  show heading.where(level: 2): set text(size: if pres { 24pt } else { 13pt }, weight: 700, fill: accent)
  show heading.where(level: 3): set text(size: if pres { 19pt } else { 11.5pt }, weight: 700, fill: ink)
  show heading: it => { it; v(if pres { 0.3em } else { 0.2em }) }

  // Figure captions
  show figure.caption: set text(size: if pres { 15pt } else { 9pt }, fill: muted)

  // Code blocks with mode-dependent rendering
  show raw.where(block: true): it => {
    set text(font: font-code, size: if pres { 15pt } else { 9pt })
    block(
      fill: faint,
      inset: if pres { 12pt } else { 10pt },
      radius: 4pt,
      width: 100%,
      stroke: 0.5pt + muted,
      // An unlabelled block may be pseudocode or an algorithm sketch;
      // do not add a misleading Language: none label.
      if it.lang == none { it } else {
        text(font: font-regular, size: if pres { 11pt } else { 8pt }, fill: muted)[_\/\/ Language: #it.lang _] + it
      },
    )
  }
  // Set the inline monospace face here as well; otherwise Typst uses
  // DejaVu Sans Mono, unlike the rest of the document. The 0.92em scale
  // compensates for JetBrains being wider than Archivo at the same
  // nominal size.
  show raw.where(block: false): it => box(
    fill: faint, inset: (x: 3pt, y: 0pt), outset: (y: 3pt), radius: 2pt,
    text(font: font-code, size: 0.92em, it),
  )

  _mode.update(pres)
  body
}

// line-sep — horizontal rule
#let line-sep() = line(length: 100%, stroke: 0.5pt + muted)

// cover — document or presentation title page
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

// section — full presentation divider or numbered A4 heading
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

// slide — one content unit, a presentation page or flowing A4 section
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

// callout — info / warn / danger / ok panel
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

// two-col — two columns, identical in both modes
#let two-col(l, r, gutter: 1.2em, columns: (1fr, 1fr)) = grid(
  columns: columns, column-gutter: gutter, l, r,
)

// stat / stats-row — number tiles scaled to the rendering mode
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

// badge — inline status, verdict or marker
// #badge("proven", kind: "ok"); kinds: info (default), ok, warn, danger, neutral.
// Use within a paragraph to distinguish established facts from assumptions
// without breaking the paragraph's flow.
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

// dtable — table in the shared theme
// Horizontal rules only, tinted header and mode-dependent inset.
// #dtable(header: ([Field], [Role]), [renewal], [the discriminator], ...)
// columns defaults to the number of header cells.
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

// diagram — captioned figure for an already loaded image
// Takes image CONTENT, not a path: Typst resolves a relative path from
// the file where it is written. Passing a path to the theme would look
// inside btyp/ rather than in the calling document. Instead use:
// #diagram(image("assets/diagrams/x.svg", width: 96%), caption: [Caption.])
// Figure caption styling comes from the theme's show rule.
#let diagram(img, caption: none) = figure(img, caption: caption)

// lead — the introductory sentence carrying the document's main point
#let lead(body) = block(
  width: 100%, inset: (left: 10pt), stroke: (left: 3pt + accent),
  text(size: 1.08em, fill: ink)[#body],
)
