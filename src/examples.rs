//! The worked example for every archetype — **in the source**.
//!
//! These were `include_str!` from an `examples/` directory, which made the
//! crate unbuildable the moment that directory was not there. It is a directory
//! of sample files: exactly the thing a repository ought to be able to drop
//! without breaking its own build, and exactly the thing `bgraph init <genre>`
//! makes redundant for a reader, since it prints one on demand.
//!
//! So the text lives here and the directory is an *output* rather than an
//! input. `bgraph init` prints from this table, the reference manual quotes
//! from it, and the suite parses, checks and renders every entry — which is
//! more coverage than the files ever had, because a file can be deleted and a
//! constant cannot.
//!
//! One entry per archetype, in the order the reference lists them.

/// A complete, working file per archetype: the genre's name and its source.
///
/// The suite renders every one of them, so this cannot hold a diagram that no
/// longer draws, and `docgen` quotes every one, so the manual cannot show a
/// genre without showing how to write it.
pub const EXAMPLES: &[(&str, &str)] = &[
    ("free", GROUPS),
    ("pie", PIE),
    ("packet", PACKET),
    ("bytefield", BYTEFIELD),
    ("treemap", TREEMAP),
    ("quadrant", QUADRANT),
    ("wave", WAVE),
    ("sankey", SANKEY),
    ("bar", BAR),
    ("xy", XY),
    ("sequence", SEQUENCE),
    ("gantt", GANTT),
    ("timeline", TIMELINE),
    ("block", BLOCK),
    ("er", ER),
    ("requirements", REQUIREMENTS),
    ("class", CLASS),
    ("kanban", KANBAN),
    ("journey", JOURNEY),
    ("radar", RADAR),
    ("rack", RACK),
    ("network", NETWORK),
    ("venn", VENN),
    ("harness", HARNESS),
    ("gitgraph", GITGRAPH),
    ("orgchart", ORGCHART),
    ("mindmap", MINDMAP),
    ("treeview", TREEVIEW),
    ("state", STATE),
    ("bpmn", BPMN),
];

const GROUPS: &str = r##"// A group is a frame sized by what it holds. Give it a placement and the
// whole subtree moves together.

graph { background: #0f172a padding: 32 gap: 44 }

style node  { fill: #1e293b stroke: #334155 1.5 font: 13 #e2e8f0 radius: 8 margin: 10 }
style group { stroke: #38bdf8 1.2 font: 12 #38bdf8 fill: #38bdf80c radius: 12 }
style edge  { stroke: #64748b 1.5 font: 11 #94a3b8 }
style store extends node { shape: cylinder stroke: #10b981 1.5 }

node client "Client"

group edge "Edge" {
  padding: 20
  node gw   "Gateway"
  node auth "Auth"
  layout {
    place gw at (0, 0)
    place auth below gw gap 26
  }
}

group core "Core services" {
  padding: 20
  node orders  "Orders"
  node billing "Billing"
  layout {
    place orders at (0, 0)
    place billing below orders gap 26
  }
}

node db "Postgres" .store

layout {
  place client at (0, 0)
  place edge right-of client gap 60
  place core right-of edge gap 60
  place db right-of core gap 60
}

client -> gw    "https"
gw     -> auth  "verify"
gw     -> orders
orders -> billing
orders -> db    "sql"
"##;

const PIE: &str = r##"type pie {
  title: "Traffic sources"
  radius: 130
  donut: 0.55
  labels: percent
}

graph { background: #0b1120 padding: 32 }

// `slice <id> ["label"] <value>` — the value rides in position.
slice organic  "Organic search" 4820
slice direct   "Direct"         2410
slice referral "Referral"       1180
slice social   "Social"          690
slice paid     "Paid"            410
"##;

const PACKET: &str = r##"type packet {
  title: "TCP header"
  width: 32
  ruler: on
}

graph { background: #0b1120 }

style field {
  fill: #1e293b
  stroke: #334155 1
  radius: 3
  font: 12 #e2e8f0
}

style flag extends field { fill: #422006 stroke: #f59e0b 1 }

// `field <id> ["label"] <bits> <bits>` — an inclusive range, in position.
field sport "Source port"           0 15
field dport "Destination port"     16 31
field seq   "Sequence number"      32 63
field ack   "Acknowledgment number" 64 95
field off   "Offset"               96 99
field rsv   "Rsvd"                100 102
field flags "Flags" .flag         103 111
field win   "Window"              112 127
field sum   "Checksum"            128 143
field urg   "Urgent pointer"      144 159
"##;

const BYTEFIELD: &str = r##"// `bytefield` is `packet` read one order of magnitude up: the ruler counts
// bytes, and a field's range is an inclusive byte range (W-039).
type bytefield {
  title: "ELF64 header"
  width: 16
  cell: 40 46
  ruler: on
}

graph { background: #0b1120 }

style field {
  fill: #1e293b
  stroke: #334155 1
  radius: 3
  font: 12 #e2e8f0
}

style magic extends field { fill: #422006 stroke: #f59e0b 1 }
style offset extends field { fill: #0c2a3e stroke: #38bdf8 1 }

// `field <id> ["label"] <bytes> <bytes>` — an inclusive range, in position.
field ident   "e_ident" .magic   0 15
field etype   "e_type"          16 17
field machine "e_machine"       18 19
field version "e_version"       20 23
field entry   "e_entry"         24 31
field phoff   "e_phoff" .offset 32 39
field shoff   "e_shoff" .offset 40 47
field flags   "e_flags"         48 51
field ehsize  "e_ehsize"        52 53
field phentsz "e_phentsize"     54 55
field phnum   "e_phnum"         56 57
field shentsz "e_shentsize"     58 59
field shnum   "e_shnum"         60 61
field shstr   "e_shstrndx"      62 63
"##;

const TREEMAP: &str = r##"type treemap {
  title: "Repository, by lines of code"
  size: 660 380
  labels: both
}
graph { background: #0b1120 padding: 30 }

group rust "Rust" {
  box lsp    "lsp.rs"        1180
  box charts "charts.rs"      760
  box solver "layout.rs"      620
  box render "render_svg.rs"  560
  box check  "check.rs"       540
  box props  "props.rs"       520
  box tests  "tests.rs"      1240
}

group grammar "Grammar" {
  box pest "syntax.pest"  70
  box ts   "grammar.js"  130
  box corp "corpus"      420
}

group docs "Docs" {
  box ref  "reference.md" 300
  box lang "language.md"  260
  box idx  "INDEX.md"     140
}
"##;

const QUADRANT: &str = r##"type quadrant {
  title: "Where the work sits"
  size: 460 400
  x-axis: "Cheap" "Costly"
  y-axis: "Low value" "High value"
  quadrants: "Do now" "Quick wins" "Drop" "Reconsider"
  labels: name
}
graph { background: #0b1120 padding: 34 }
style point { font: 12 #e2e8f0 }

point wave     "type wave"     0.62 0.86
point sankey   "type sankey"   0.28 0.72
point quadrant "type quadrant" 0.16 0.64
point block    "type block"    0.70 0.55
point gantt    "type gantt"    0.80 0.30
point venn     "Venn"          0.35 0.18
"##;

const WAVE: &str = r##"type wave {
  title: "SPI transfer"
  period: 34
  lane: 26 16
  every: 1
}
graph { background: #0b1120 padding: 30 }
style signal { font: 12 #e2e8f0 stroke: #7dd3fc }

signal sclk "SCLK"  "p........."
signal cs   "~CS"   "10........1"
signal mosi "MOSI"  "x.=.=.=.=.x" { data: "cmd" "addr" "data" "crc" }
signal miso "MISO"  "z...=.=...z" { data: "ack" "status" }
signal busy "BUSY"  "0..1...0..."
"##;

const SANKEY: &str = r##"type sankey {
  title: "Where the traffic goes"
  size: 620 340
  gap: 16
  labels: both
}
graph { background: #0b1120 padding: 32 }
style node { font: 12 #e2e8f0 }

node search "Organic"
node social "Social"
node paid   "Paid"
node landing "Landing"
node pricing "Pricing"
node signup  "Sign-up"
node bounce  "Bounce"

search  -> landing 4200
social  -> landing 1500
paid    -> pricing  900
search  -> pricing  620
landing -> signup  2100
landing -> bounce  3600
pricing -> signup  1100
pricing -> bounce   420
"##;

const BAR: &str = r##"type bar {
  title: "Quarterly revenue, by region"
  size: 520 280
  categories: "Q1" "Q2" "Q3" "Q4"
}
graph { background: #0b1120 padding: 32 }
style series { font: 12 #e2e8f0 }

series emea "EMEA"   120 185 240 310
series amer "AMER"   200 210 265 280
series apac "APAC"    80 130 175 260
"##;

const XY: &str = r##"type xy {
  title: "Requests per second, by week"
  size: 520 260
  categories: "W1" "W2" "W3" "W4" "W5" "W6" "W7" "W8"
  mark: line
}
graph { background: #0b1120 padding: 32 }
style series { font: 12 #e2e8f0 }

series edge   "Edge"    120 168 155 210 265 240 310 355
series origin "Origin"   90 102 118 130 126 148 160 172
series cache  "Cache"    30  66  37  80 139  92 150 183
"##;

const SEQUENCE: &str = r##"type sequence {
  title: "Checkout"
  column: 160
  row: 46
  numbered: on
}
graph { background: #0b1120 padding: 30 }
style participant { fill: #1e293b stroke: #334155 1.5 font: 13 #e2e8f0 radius: 8 margin-bottom: 12 }
style edge  { stroke: #7dd3fc 1.5 font: 11 #cbd5e1 }
style group { stroke: #f59e0b 1.2 font: 11 #f59e0b fill: #f59e0b10 margin: 16 8 }

participant browser "Browser"
participant api     "API"
participant bank    "Bank"
participant db      "Postgres"

browser -> api  "POST /checkout"

// The box down `api`'s lifeline while the call is outstanding (W-047).
activate api {
  api -> db   "reserve stock"
  db  --> api "ok"
}

group retry "retry up to 3×" {
  api  -> bank "authorise"
  bank --> api "declined" { stroke: #f43f5e }
}

api  -> bank "authorise"
bank --> api "approved"
api  -> db   "commit"
api  --> browser "201 Created"
"##;

const GANTT: &str = r##"type gantt {
  title: "bgraph, the archetypes"
  size: 560 26
  today: 2026-09-14
}
graph { background: #0b1120 padding: 32 }
style task  { font: 12 #e2e8f0 }
style group { stroke: #334155 1 font: 11 #94a3b8 fill: #ffffff06 radius: 4 }

group found "foundations" {
  task vocab  "vocabulary"   2026-08-03 6d
  task groups "containers"   2026-08-09 5d
  task frame  "plot frame"   2026-08-14 4d
  task dates  "dates"        2026-08-18 3d
}

group charts "charts" {
  task pie  "pie"      2026-08-21 3d
  task bars "bar & xy" 2026-08-24 6d
  task tree "treemap"  2026-08-30 4d
  task flow "sankey"   2026-09-03 5d
}

group dev "software" {
  task seq   "sequence" 2026-09-08 5d
  task state "state"    2026-09-13 4d
  task uml   "class"    2026-09-17 4d
  task plan  "gantt"    2026-09-21 2026-09-28
}
"##;

const TIMELINE: &str = r##"type timeline { title: "Rust editions and releases" size: 860 36 }
graph { background: #ffffff padding: 32 }

event r1  "Rust 1.0"        2015-05-15
event e15 "Edition 2015"    2015-05-15
event e18 "Edition 2018"    2018-12-06
event async "async/await"   2019-11-07
event e21 "Edition 2021"    2021-10-21
event gats "GATs stabilised" 2022-11-03
event lets "let-else"       2022-06-30
event e24 "Edition 2024"    2025-02-20
"##;

const BLOCK: &str = r##"type block { title: "Internet banking — containers" }
graph { background: #f8fafc padding: 36 gap: 72 }
style block { margin: 10 }

block customer "Personal Banking Customer" person {
  description: "A customer of the bank, with personal accounts."
}

group system "Internet Banking System" {
  block web "Web Application" container {
    tech: "Java, Spring MVC"
    description: "Delivers the static content and the SPA."
  }
  block spa "Single-Page App" container {
    tech: "TypeScript, Angular"
    description: "Provides all the banking functionality in the browser."
  }
  block api "API Application" container {
    tech: "Java, Spring MVC"
    description: "Provides banking functionality via a JSON/HTTPS API."
  }
  block db "Database" database {
    tech: "Oracle 19c"
    description: "Stores user registration, hashed credentials and logs."
  }
  layout {
    place web at (0, 0)
    place spa right-of web gap 150
    place api below spa gap 64
    place db left-of api gap 150
  }
}

block mainframe "Mainframe Banking System" external {
  description: "Stores the core banking information."
}

layout {
  place customer at (0, 0)
  place system below customer gap 52
  place mainframe below db gap 56
}

customer -> web "Visits bigbank.com" { tech: "HTTPS" }
web -> spa "Delivers to the browser"
spa -> api "Makes API calls to" { tech: "JSON/HTTPS" }
api -> db "Reads from and writes to" { tech: "SQL/TCP" }
api -> mainframe "Makes API calls to" { tech: "XML/HTTPS" }
"##;

const ER: &str = r##"type er { title: "Ordering schema" }
graph { background: #0b1120 padding: 32 gap: 56 }

style entity { fill: #1e293b stroke: #38bdf8 1.4 font: 13 #e2e8f0 radius: 6 padding: 14 margin: 10 }
style edge   { stroke: #64748b 1.4 font: 11 #94a3b8 head: none }

entity customer "Customer" {
  - id: uuid PK
  - email: text
  - created_at: timestamptz
}

entity order "Order" {
  - id: uuid PK
  - customer_id: uuid FK
  - placed_at: timestamptz
  - total_cents: bigint
}

entity line "OrderLine" {
  - order_id: uuid FK
  - sku: text
  - qty: integer
}

entity address "Address" {
  - customer_id: uuid FK
  - line1: text
  - city: text
}

layout {
  place customer at (0, 0)
  place order right-of customer gap 120
  place line below order gap 56
  place address below customer gap 56
}

customer -> order   "places"    one many
order    -> line    "contains"  one one-many
customer -> address "ships to"  one zero-many
"##;

const REQUIREMENTS: &str = r##"// `requirements` is a class box with a relation vocabulary: a bare line in the
// block is a line of the requirement, and a link says what it asserts (W-051).
type requirements { title: "Braking system" }
graph { background: #0b1120 padding: 32 gap: 64 }

style requirement { fill: #1e293b stroke: #38bdf8 1.4 font: 13 #e2e8f0 radius: 6 padding: 14 margin: 10 }
style edge        { stroke: #64748b 1.4 font: 11 #94a3b8 head: open dash: dashed }

requirement stop "Stopping distance" {
  - REQ-001
  - The vehicle shall stop within 40 m from 100 km/h on dry asphalt.
  risk: high
  verify: test
}

requirement fade "Fade resistance" {
  - REQ-002
  - Braking performance shall not drop by more than 10% over ten consecutive stops.
  risk: medium
  verify: test
}

requirement pedal "Pedal force" {
  - REQ-003
  - Full braking shall require no more than 500 N of pedal force.
  risk: low
  verify: demonstration
}

requirement adas "Emergency assist" {
  - REQ-004
  - The assist shall command full braking within 150 ms of a detected obstacle.
  risk: high
  verify: analysis
}

layout {
  place stop at (0, 0)
  place fade below stop gap 56
  place pedal right-of stop gap 150
  place adas below pedal gap 56
}

// `<from> -> <to> ["label"] <link>` — read tail to head.
fade  -> stop  derive
pedal -> stop  refine
adas  -> stop  satisfy
adas  -> pedal trace
"##;

const CLASS: &str = r##"type class { title: "Ordering domain" }
graph { background: #0b1120 padding: 32 gap: 56 }

style class { fill: #1e293b stroke: #38bdf8 1.4 font: 13 #e2e8f0 radius: 6 padding: 14 margin: 10 }
style edge  { stroke: #64748b 1.4 font: 11 #94a3b8 }

class user "User" {
  - id: Uuid
  - email: String
  - name: String
}

class order "Order" {
  - id: Uuid
  - user_id: Uuid
  - total: Money
  - placed_at: DateTime
}

class line "OrderLine" {
  - order_id: Uuid
  - sku: String
  - qty: u32
}

class money "Money" {
  - amount: i64
  - currency: Iso4217
}

layout {
  place user at (0, 0)
  place order right-of user gap 90
  place line below order gap 44
  place money right-of order gap 90
}

user  -> order "places"
order -> line  "contains"
order -> money "totals" { dash: dashed }
"##;

const KANBAN: &str = r##"// A board: `group`s are the columns, in declaration order, and the cards
// inside stack in declaration order. A column may carry a WIP `limit` (W-050).
type kanban {
  title: "Sprint 14"
  column: 210
}

graph { background: #0b1120 padding: 28 }

style card    { fill: #1e293b stroke: #334155 1 radius: 5 font: 12 #e2e8f0 }
style group   { fill: #0f172a stroke: #1e293b 1 radius: 8 dash: solid font: 12 #94a3b8 }
style blocked extends card { fill: #422006 stroke: #f59e0b 1 }

group todo "To do" {
  card ruler   "Byte-granular ruler for wire formats"
  card venn    "Circle geometry for overlapping sets"
  card metrics "Real font metrics instead of the estimate"
}

// Deliberately over its limit: the warning firing is the point of the genre.
group doing "In progress" {
  limit: 2
  card board "Columns of cards, sized by what they hold"
  card links "Typed relations between requirements"
  card fonts "Measure text instead of guessing at it" .blocked
}

group review "In review" {
  limit: 3
  card scatter "Coordinates instead of positions"
}

group done "Done" {
  card offset "Manual escape hatch out of label placement"
  card bytes  "Wire formats at byte granularity"
}
"##;

const JOURNEY: &str = r##"// Steps in the order they are taken, one line per actor — the point being the
// step where two people diverge (W-052).
type journey {
  title: "Returning an order"
  actors: "Customer" "Support agent"
  size: 620 200
  range: 1 5
}

graph { background: #0b1120 padding: 30 }
style step { font: 11 #cbd5e1 }
style group { stroke: #334155 1 fill: #1e293b60 radius: 5 font: 11 #94a3b8 dash: solid }

group find "Find the order" {
  step login  "Sign in"          4 5
  step locate "Locate the order" 3 5
}

group ask "Ask for the return" {
  step form   "Fill the form"    2 4
  step wait   "Wait for a reply" 1 3
}

group close "Close it" {
  step label  "Print the label"  3 4
  step refund "Refund lands"     5 5
}
"##;

const RADAR: &str = r##"// Axes radiating from a centre, one closed polygon per series. Every axis
// shares one scale, or the shape a reader compares would be a lie (W-046).
type radar {
  title: "Two engine designs"
  categories: "Power" "Economy" "Range" "Weight" "Cost" "Noise"
  radius: 140
  range: 0 100
}

graph { background: #0b1120 padding: 32 }
style series { font: 11 #cbd5e1 }

series turbo   "Turbocharged"  92 48 70 55 40 45
series electric "Electric"     78 95 52 40 62 96
"##;

const RACK: &str = r##"// A rack elevation. Units count from 1 at the bottom, as a rack is labelled —
// and a device occupies an inclusive span of them (W-044). A `group` is one
// rack, so a bank is a row of them: they stand on one floor, and a short rack
// is short at the top (W-132).
type rack {
  title: "Row A — hall 1"
  units: 24
  cell: 230 19
  ruler: on
}

graph { background: #0b1120 padding: 28 }

style device  { fill: #1e293b stroke: #334155 1 radius: 3 font: 11 #e2e8f0 }
style network extends device { fill: #0c2a3e stroke: #38bdf8 1 }
style power   extends device { fill: #422006 stroke: #f59e0b 1 }
style group   { stroke: #475569 1 }
// The diagram's own furniture, not any device's: `grid` is the unit rules and
// the U numbers, `title` is the heading (W-134).
style grid    { stroke: #1e293b 0.5 font: 9 #64748b }
style title   { font: 16 #f8fafc }

// `device <id> ["label"] <lowest> <highest>`
group a1 "A1 — general" {
  device tor1   "Top-of-rack switch"  .network 24 24
  device patch1 "Patch panel"         .network 23 23
  device proc01 "rk1-app01"                    19 22
  device proc02 "rk1-app02"                    15 18
  device cap01  "rk1-db01"                     11 14
  device store1 "Storage shelf"                 6  9
  device ups1   "UPS"                 .power    1  4
}

group a2 "A2 — compute" {
  device tor2   "Top-of-rack switch"  .network 24 24
  device proc03 "rk2-app01"                    18 21
  device proc04 "rk2-app02"                    14 17
  device gpu01  "rk2-gpu01"                     8 11
  device ups2   "UPS"                 .power    1  4
}

// A rack of its own height: the network rack is half a cabinet.
group a3 "A3 — edge" {
  units: 12
  device edge1  "Border router"       .network 12 12
  device edge2  "Aggregation switch"  .network 10 11
  device fw     "Firewall"                      6  7
}
"##;

const NETWORK: &str = r##"// Topology: hosts, links and the segments that hold them. The role picks the
// shape; placement stays declared, as in `free` (W-043).
type network { title: "One site — edge to core" }
graph { background: #0b1120 padding: 30 gap: 70 }

style host  { fill: #1e293b stroke: #334155 1.2 font: 12 #e2e8f0 radius: 5 margin: 10 }
style edge  { stroke: #64748b 1.3 font: 10 #94a3b8 head: none }
style group { stroke: #334155 1 fill: #0f172a80 radius: 8 dash: dashed font: 11 #94a3b8 }

host net "Internet" cloud
host fw  "Edge firewall" firewall { addr: "203.0.113.1" }
host tor "Top-of-rack" switch { addr: "10.0.0.1" }

group core "Core segment — 10.0.0.0/24" {
  host proc01 "rk1-app01" server { addr: "10.0.0.11" }
  host proc02 "rk1-app02" server { addr: "10.0.0.12" }
  host shelf  "Storage shelf"   storage { addr: "10.0.0.20" }
}

layout {
  place net at (0, 0)
  place fw below net gap 60
  place tor below fw gap 60
  place proc01 right-of tor gap 190
  place proc02 below proc01 gap 40
  place shelf below proc02 gap 40
}

net -> fw  "uplink" "1 GbE"
fw  -> tor "trunk"  "10 GbE"
// The link to the first server has its label moved along the line: the
// switch's margin leaves the three labels little room, and the one that
// belongs to this line would otherwise sit on the server it names.
tor -> proc01 "" "10 GbE" { label-offset: -40 0 }
tor -> proc02 "" "10 GbE"
// Told which side to leave by (`D-033`). Three links off one switch share its
// right-hand side; sending the longest one out of the bottom keeps its label
// off the switch and its line out of the other two's way.
tor -> shelf  "" "10 GbE" { from: bottom to: left }
"##;

const VENN: &str = r##"// Two sets have an exact lens area. Three sets fit the pairwise areas when
// possible, but remain indicative: the seven regions are not solved.
// A/B/C identify the sets. The key lists totals and named pairwise overlaps;
// a pair's total includes members also in the third set. Omitted links mean 0.
//
// **On a white surface on purpose**, and one of only two examples that is
// (`Q-006`). A Venn is read through its overlaps, which are translucent fills
// composited on top of each other; on a dark surface each layer darkens the
// one beneath and the three-way region goes to near black. Light is the only
// surface on which "more layers" reads as "more".
type venn {
  title: "Who reads which diagram"
  radius: 105
}

graph { background: #ffffff padding: 34 }
style set { font: 12 "Archivo" #1a1a1a }

set dev  "Developers" 420
set ops  "Operations" 260
set arch "Architects" 180

// `<from> -> <to> <shared>` — how many things the two sets have in common.
dev  -> ops  120
ops  -> arch  70
dev  -> arch 140
"##;

const HARNESS: &str = r##"// A connector's pins are its member lines; a wire is an edge carrying the pin
// it lands on at each end (D-025, W-045).
type harness { title: "Sensor loom — J1 to J2" }
graph { background: #0b1120 padding: 32 gap: 160 }

style connector { fill: #1e293b stroke: #38bdf8 1.4 font: 12 #e2e8f0 radius: 5 padding: 12 margin: 10 }
style edge      { stroke: #64748b 1.3 font: 10 #94a3b8 head: none }
style power     { stroke: #f43f5e 1.6 font: 10 #fda4af head: none }
style ground    { stroke: #64748b 1.6 font: 10 #94a3b8 head: none dash: dashed }

connector j1 "J1 — MOLEX 4-way" {
  - 1: +12V
  - 2: GND
  - 3: SIG_A
  - 4: SIG_B
}

connector j2 "J2 — JST 6-way" {
  - 1: VCC
  - 2: RETURN
  - 3: IN_A
  - 4: IN_B
  - 5: SHIELD
  - 6: n/c
}

layout {
  place j1 at (0, 0)
  place j2 right-of j1
}

// `<from> -> <to> ["label"] <from-pin> <to-pin>`
j1 -> j2 "power"  .power  1 1
j1 -> j2 "ground" .ground 2 2
j1 -> j2 "sig A"          3 3
j1 -> j2 "sig B"          4 4
"##;

const GITGRAPH: &str = r##"// Parentage is the edges; the rails are the archetype's to assign. The author
// still declares every commit and what came before it (W-049).
type gitgraph {
  title: "A release, with a hotfix"
  branches: "main" "feature" "hotfix"
  spacing: 96 62
}

graph { background: #0b1120 padding: 30 }
style commit { font: 11 #e2e8f0 }

// `commit <id> ["message"] <branch>`
commit c1 "init"        "main"
commit c2 "parser"      "main" { tag: "v0.1" }
commit f1 "scatter"     "feature"
commit f2 "radar"       "feature"
commit c3 "merge"       "main"
commit h1 "fix the U1"  "hotfix"
commit c4 "merge fix"   "main" { tag: "v0.2" }

c1 -> c2
c2 -> f1
f1 -> f2
c2 -> c3
f2 -> c3
c3 -> h1
c3 -> c4
h1 -> c4
"##;

const ORGCHART: &str = r##"// The hierarchy is the edges; the archetype places every box, so adding a
// report never means renumbering anything (W-041).
type orgchart { title: "Broadcast engineering" row: 96 spacing: 26 }
graph { background: #0b1120 padding: 30 }
style position { fill: #1e293b stroke: #38bdf8 1.3 font: 12 #e2e8f0 radius: 6 padding: 12 }

position cto  "CTO"
position plat "Platform lead"
position sig  "DSP lead"
position ops  "Operations lead"
position k8s  "Kubernetes"
position api  "Backend"
position dsp1 "Signal chain"
position dsp2 "Capture"
position noc  "NOC"

cto -> plat
cto -> sig
cto -> ops
plat -> k8s
plat -> api
sig -> dsp1
sig -> dsp2
ops -> noc
"##;

const MINDMAP: &str = r##"// The same tree as the orgchart, arranged outward — the root stays central
// however lopsided the branches are (W-041).
type mindmap { title: "What a diagram tool has to get right" row: 150 spacing: 22 }
graph { background: #0b1120 padding: 30 }
style topic { fill: #1e293b stroke: #38bdf8 1.3 font: 12 #e2e8f0 radius: 14 padding: 11 }

topic root "A diagram tool"
topic lay  "Layout"
topic sty  "Style"
topic chk  "Diagnostics"
topic edit "Editor"
topic decl "Declared, not solved"
topic anch "Relative anchors"
topic casc "A real cascade"
topic hard "Unknown names are errors"
topic span "Every error has a span"
topic lsp  "One validator, two front ends"

root -> lay
root -> sty
root -> chk
root -> edit
lay -> decl
lay -> anch
sty -> casc
chk -> hard
chk -> span
edit -> lsp
"##;

const TREEVIEW: &str = r##"// The same tree as indented rows — depth is the indent, order is the order
// written (W-041).
type treeview { title: "src/" row: 26 spacing: 14 }
graph { background: #0b1120 padding: 28 }
style entry { fill: #1e293b stroke: #334155 1 font: 12 #e2e8f0 radius: 4 padding: 8 }

entry src     "src/"
entry lib     "lib.rs"
entry parser  "parser.rs"
entry check   "check.rs"
entry solver  "layout.rs"
entry charts  "charts.rs"
entry render  "render_svg.rs"
entry lsp     "lsp.rs"

src -> lib
src -> parser
src -> check
src -> solver
solver -> charts
solver -> render
src -> lsp
"##;

const STATE: &str = r##"// The gaps are wide where the transition carries a long label: a guard is
// text on the line, and a gap narrower than it lets the next box paint over
// it. `check` says so rather than widening anything itself — moving a box is
// the author's call (`D-001`).
type state { title: "Order lifecycle" }
graph { background: #0b1120 padding: 32 gap: 54 }

style state { fill: #1e293b stroke: #38bdf8 1.5 font: 13 #e2e8f0 padding: 16 margin: 10 }
style edge  { stroke: #64748b 1.5 font: 11 #94a3b8 }
style group { stroke: #f59e0b 1.2 font: 11 #f59e0b fill: #f59e0b0c }

state begin      { kind: initial }
state draft "Draft"
state placed "Placed"

group fulfil "fulfilment" {
  padding: 22
  state picking "Picking"
  state shipped "Shipped"
}

state done "Delivered"
state stop { kind: final }
state cancelled "Cancelled"

layout {
  place begin at (0, 0)
  place draft right-of begin gap 46
  place placed right-of draft
  place picking right-of placed gap 150
  place shipped right-of picking gap 80
  place done right-of fulfil gap 56
  place stop right-of done gap 46
  place cancelled below placed gap 62
}

begin   -> draft
draft   -> placed  "submit"
placed  -> picking "pay"      { guard: "funds cleared" }
picking -> shipped "dispatch"
shipped -> done    "deliver"
done    -> stop
placed  -> cancelled "cancel"
"##;

const BPMN: &str = r##"// A business process. Four words, because four things have four positional
// shapes: a task has a kind, an event has a stage and a trigger, a gateway has
// a gate, a datum has an artefact. Placement is declared as it is everywhere
// else — the archetype supplies the notation, the author says where it goes.
//
// **On a white surface on purpose**, and one of only two examples that is
// (`Q-006`). BPMN is a notation people already know from somewhere else, and
// everywhere else draws it black on white; a process diagram in this project's
// house dark would be the first thing a reader has to translate.
type bpmn { title: "Expense claim" }
graph { background: #ffffff padding: 36 gap: 40 }

style node  { font: 13 #111827 stroke: #334155 1.2 fill: #ffffff margin: 10 }
style edge  { stroke: #475569 1.3 font: 11 #475569 }
style store { fill: #f1f5f9 }

group filed "Employee" {
  padding: 22

  event submit "Claim submitted" start message
  task  fill   "Fill the form"   user
  layout {
    place submit at (0, 0)
    place fill right-of submit gap 44
  }
}

group review "Finance" {
  padding: 22

  task    check "Validate receipts" rule
  gateway ok    "Within policy?"    exclusive
  task    pay   "Pay the claim"     service
  task    query "Ask for detail"    send
  event   done  "Reimbursed"        end
  layout {
    place check at (0, 0)
    place ok right-of check gap 52
    place pay right-of ok gap 52
    place query below pay gap 74
    place done right-of pay gap 52
  }
}

// A boundary event sits *on* a task, and says so by naming it. Placement, and
// declared like every other placement.
event late "14 days" boundary timer { attach: pay }

// The form sits between the two pools, over the task that reads it. Put to the
// right of the employee's pool — which is where it went first — its message
// flow had to cross the whole picture to reach `check` on the far left, which
// is a worse crossing than the one that move was meant to remove.
data claim  "Claim form" object
data ledger "Ledger"     store

layout {
  place review below filed gap 112
  place claim below filed gap 22 align left
  place ledger below done gap 40
}

submit -> fill
fill   -> check
check  -> ok
ok     -> pay   "within"
ok     -> query "over"    default
query  -> check
pay    -> done
late   -> query

// A message crosses between participants; an association only relates.
fill  -> claim  association
pay   -> ledger association
claim -> check  message
"##;
