# Files, components and data

Start with a single `.bd` file. When several diagrams share a style or data
source, the features below help avoid copying the same content.

All commands on this page use the command-line renderer, which can read
files. Keep the example files together in one working folder.

## Share styles

Create `house.bds`:

```text
style service { fill: #dbeafe stroke: #2563eb 1.5 radius: 8 }
```

Then create `application.bd` in the same folder:

```text
include "house.bds"

node api "API" .service
node jobs "Background jobs" .service
layout { place jobs below api gap 40 }
api -> jobs
```

Render it:

```sh
bgraph application.bd -o application.svg
```

An `include` path is relative to the file containing that statement.
A circular include or a duplicate declaration produces an error.
A `.bds` file contains styles only; a `.bd` file can contain a full diagram.

## Include a diagram under a prefix

To include the previous diagram from `system.bd`:

```text
include "house.bds"
include "application.bd" as backend

node ui "User interface"
layout { place ui above backend.api gap 48 }
ui -> backend.api
```

Render with `bgraph system.bd -o system.svg`.
`backend.api` refers to `api` in the included file. Node references receive
the same prefix, which avoids identifier collisions between diagrams.
Keep `house.bds` included at the top level too: this makes the shared
`.service` class available when the application is included under a prefix.

## Reuse a component

A component is a reusable diagram fragment. Save this complete example as
`components.bd`:

```bgraph
component service {
  node app "{{label}}"
  node db "Database"
  layout { place db below app gap 32 }
  app -> db
}

use service as web { label: "Web" }
use service as jobs { label: "Background jobs" }

layout {
  place web at (0, 0)
  place jobs right-of web gap 80
}

web.app -> jobs.app "triggers"
```

Render it with `bgraph components.bd -o components.svg`.
Each instance is a frame you can place. Parameters are substituted by the
template engine with escaping appropriate to their context. Notes and legends
inside a component must use a relative anchor to one of its elements, so they
follow each instance. Nested `component`, `use`, `include`, `type` and `graph`
statements are rejected inside component bodies.

## Fill a template

A template contains holes such as `{{year}}`, filled from supplied values.
Create `traffic.bd`:

```text
type pie { title: "Traffic in {{year}}" }
slice organic "Organic search" {{organic}}
slice direct "Direct" {{direct}}
slice referral "Referrals" {{referral}}
```

Supply values on the command line:

```sh
bgraph traffic.bd -o traffic.svg --set year=2026 --set organic=4820 --set direct=2410 --set referral=1180
```

Or create `traffic.json`:

```json
{"year": 2026, "organic": 4820, "direct": 2410, "referral": 1180}
```

Then render or inspect the filled source:

```sh
bgraph traffic.bd --data traffic.json -o traffic.svg
bgraph traffic.bd --data traffic.json --emit
bgraph watch traffic.bd --data traffic.json --open --port 0
```

`--emit` prints the filled source. `watch --data` reloads changed data as
well as the diagram and the files selected by the filled template. A missing
value produces an error naming the hole; a missing field in a repeated row
also identifies the row. `--emit` deliberately allows partial substitution.
Filled templates support the same SVG, PDF, PNG and JSON outputs, themes
and fonts as ordinary source files.

## Load chart data from CSV

For a pie chart, create `sales.csv`:

```csv
name,value
Organic search,4820
Direct,2410
Referrals,1180
```

Create `sales.bd` in the same folder:

```text
type pie { title: "Sales" from: "sales.csv" }
```

Render it:

```sh
bgraph sales.bd -o sales.svg
```

Required columns depend on the type; look up its section in the
[reference](reference.md). To repeat a template line for each record,
see `--rows` in `bgraph --help` and
[the repetition syntax](reference.md#repetition). Both CSV entry points accept
quoted commas, escaped quotes and quoted multiline cells. Duplicate or empty
column names and rows with the wrong number of cells produce an error.

!!! note "Embedded rendering and files"
    `include` and `from:` require a disk path. Use the CLI or Rust's
    `render_file`. The current-text renderer in the browser, VS Code pane
    and Typst plugin has no filesystem context. For a composed diagram in
    a document, generate its SVG with the CLI first.
