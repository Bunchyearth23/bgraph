use super::*;

/// A server holding one document, and the params a request would carry —
/// the tests below drive the real handlers rather than their innards.
fn server(text: &str) -> Server {
    let mut docs = HashMap::new();
    docs.insert("file:///t.bd".to_string(), text.to_string());
    Server { docs }
}

fn at(text: &str, offset: usize) -> Value {
    let (line, character) = LineIndex::new(text).line_col(offset);
    json!({
        "textDocument": { "uri": "file:///t.bd" },
        "position": { "line": line, "character": character },
    })
}

#[test]
fn line_index_round_trips_utf16_positions() {
    let text = "node a\nnode é \"héllo\"\nnode c";
    let index = LineIndex::new(text);

    for offset in 0..=text.len() {
        if !text.is_char_boundary(offset) {
            continue;
        }
        let (line, col) = index.line_col(offset);
        assert_eq!(index.offset(line, col), offset, "offset {offset}");
    }
}

#[test]
fn enclosing_block_identifies_the_owner() {
    let text = "graph {\n  pad";
    assert_eq!(Context::at(text, text.len()).block, Some(Block::Graph));

    let text = "style x {\n  fill";
    assert_eq!(
        Context::at(text, text.len()).block,
        Some(Block::Style(None))
    );

    let text = "node a \"A\" {\n  fill";
    assert_eq!(Context::at(text, text.len()).block, Some(Block::Node));

    let text = "a -> b {\n  rou";
    assert_eq!(Context::at(text, text.len()).block, Some(Block::Edge));

    let text = "style node {\n  fill";
    assert_eq!(
        Context::at(text, text.len()).block,
        Some(Block::Style(Some("node")))
    );

    let text = "style edge {\n  rou";
    assert_eq!(
        Context::at(text, text.len()).block,
        Some(Block::Style(Some("edge")))
    );

    // The three heads the grammar gives a `group_block`.
    for text in [
        "group g \"G\" {\n  ",
        "component c {\n  ",
        "activate p {\n  ",
    ] {
        assert_eq!(
            Context::at(text, text.len()).block,
            Some(Block::Container),
            "{text:?}"
        );
    }

    let text = "node a\nlayout {\n  ";
    assert_eq!(Context::at(text, text.len()).block, Some(Block::Layout));

    // A `use` takes the ordinary block — properties only.
    let text = "use c as c1 {\n  ";
    assert_eq!(Context::at(text, text.len()).block, Some(Block::Node));

    let text = "graph { padding: 4 }\nnode a";
    assert_eq!(Context::at(text, text.len()).block, None);
}

#[test]
fn completion_inside_layout_offers_only_canonical_relations() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "node a\nnode b\nlayout {\n  ";
    let labels: Vec<String> = server
        .items_for(text, "  ", text.len())
        .iter()
        .filter_map(|item| item["label"].as_str().map(str::to_string))
        .collect();
    assert!(labels.contains(&"place".to_string()), "{labels:?}");
    assert!(labels.contains(&"equal-width".to_string()), "{labels:?}");
    assert!(!labels.contains(&"node".to_string()), "{labels:?}");

    let targets: Vec<String> = server
        .items_for(text, "  place ", text.len())
        .iter()
        .filter_map(|item| item["label"].as_str().map(str::to_string))
        .collect();
    assert!(
        targets.contains(&"a".to_string()) && targets.contains(&"b".to_string()),
        "{targets:?}"
    );
}

#[test]
fn completion_inside_a_local_layout_stays_with_its_direct_members() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "node outside\ngroup service {\n  node api\n  node db\n  layout {\n    place api at (0, 0)\n  }\n}\n";
    let offset = text.find("place api").unwrap() + "place ".len();
    let targets: Vec<String> = server
        .items_for(text, "    place ", offset)
        .iter()
        .filter_map(|item| item["label"].as_str().map(str::to_string))
        .collect();
    assert!(targets.contains(&"api".to_string()), "{targets:?}");
    assert!(targets.contains(&"db".to_string()), "{targets:?}");
    assert!(!targets.contains(&"outside".to_string()), "{targets:?}");
}

#[test]
fn braces_inside_strings_and_comments_are_ignored() {
    let text = "node a \"{\"\nnode b";
    assert_eq!(Context::at(text, text.len()).block, None);

    let text = "// a { comment\nnode b";
    assert_eq!(Context::at(text, text.len()).block, None);
}

#[test]
fn declared_lists_nodes_and_styles_with_labels() {
    let text = "style dark {\n fill: #000\n}\nnode a \"Alpha\"\nnode b\n";
    assert_eq!(
        declared(text, "node"),
        vec![
            ("a".to_string(), "Alpha".to_string()),
            ("b".to_string(), String::new())
        ]
    );
    assert_eq!(declared(text, "style")[0].0, "dark");
}

#[test]
fn declaration_span_points_at_the_identifier() {
    let text = "node alpha \"A\"\nnode beta \"B\"\n";
    let span = declaration_span(text, "node", "beta").expect("beta is declared");
    assert_eq!(&text[span.start..span.end], "beta");
}

#[test]
fn word_at_grabs_hyphenated_identifiers() {
    let text = "node a right-of b";
    let (word, _) = word_at(text, 10).expect("cursor is inside a word");
    assert_eq!(word, "right-of");
}

#[test]
fn diagnostics_carry_positions() {
    let text = "node a .ghost\n";
    let diags = diagnostics(text);
    assert_eq!(diags.len(), 1);
    assert_eq!(diags[0]["severity"], 1);
    assert_eq!(diags[0]["range"]["start"]["line"], 0);
    assert_eq!(diags[0]["range"]["start"]["character"], 8);
    assert!(
        diags[0]["message"]
            .as_str()
            .unwrap()
            .contains("unknown style")
    );
}

#[test]
fn a_syntax_error_is_reported_at_its_location() {
    let diags = diagnostics("node a {\n");
    assert_eq!(diags.len(), 1);
    assert!(
        diags[0]["message"]
            .as_str()
            .unwrap()
            .contains("syntax error")
    );
}

#[test]
fn completion_offers_styles_after_a_dot() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "style dark {\n fill: #000\n}\nnode a .";
    let items = server.items_for(text, "node a .", text.len());
    assert!(items.iter().any(|i| i["label"] == "dark"));
}

#[test]
fn completion_offers_node_ids_after_an_anchor_keyword() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "node alpha\nnode beta right-of ";
    let items = server.items_for(text, "node beta right-of ", text.len());
    assert!(items.iter().any(|i| i["label"] == "alpha"));
}

#[test]
fn completion_offers_properties_inside_a_block_by_context() {
    let server = Server {
        docs: HashMap::new(),
    };

    let text = "node a {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"shape"));
    assert!(!labels.contains(&"router"), "router is edge-only");

    let text = "a -> b {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"router"));
    assert!(!labels.contains(&"shape"), "shape is node-only");

    let text = "graph {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"flow"));
    assert!(!labels.contains(&"fill"));
}

/// A group is where a diagram nests, and completion used to treat it as a
/// node — properties only, no way to declare anything inside it without
/// knowing the word already (`W-131`).
#[test]
fn completion_inside_a_group_offers_statements_as_well_as_properties() {
    let server = Server {
        docs: HashMap::new(),
    };

    for (what, text) in [
        ("group", "type network\ngroup g \"G\" {\n  "),
        ("component", "type network\ncomponent c {\n  "),
        ("activate", "type sequence\nactivate p {\n  "),
    ] {
        let items = server.items_for(text, "  ", text.len());
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        let item = props::archetype(declared_archetype(text)).unwrap().item;
        assert!(
            labels.contains(&item),
            "{what}: no `{item}` to declare with"
        );
        assert!(labels.contains(&"group"), "{what}: groups do not nest");
        assert!(labels.contains(&"padding"), "{what}: lost its properties");
    }

    // And the two lists keep their own order: declaring comes first.
    let text = "type network\ngroup g \"G\" {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let sort = |label: &str| -> String {
        items
            .iter()
            .find(|i| i["label"] == label)
            .and_then(|i| i["sortText"].as_str())
            .unwrap_or("")
            .to_string()
    };
    assert!(sort("host") < sort("padding"));
}

/// An anchor names something in scope, and the things in scope are mostly
/// the group's own members.
#[test]
fn completion_inside_a_group_offers_its_members_as_anchors() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "type network\ngroup g \"G\" {\n  host a \"A\"\n  host b right-of ";
    let items = server.items_for(text, "  host b right-of ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"a"));
}

/// `style node` and `style edge` are themes for one side of the diagram,
/// and `check_styles` says so with a warning. Completion says so by not
/// offering the other side at all.
#[test]
fn a_themed_style_is_offered_only_its_own_side() {
    let server = Server {
        docs: HashMap::new(),
    };

    let text = "style node {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"shape"));
    assert!(
        !labels.contains(&"router"),
        "an edge property in `style node`"
    );

    let text = "style edge {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"router"));
    assert!(
        !labels.contains(&"shape"),
        "a node property in `style edge`"
    );

    // A style under any other name is generic, and takes both.
    let text = "style warn {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"shape") && labels.contains(&"router"));
}

/// A theme reads a named part of the vocabulary, so offering the rest is
/// offering the warning `check_styles` is about to write.
#[test]
fn a_theme_is_offered_only_what_it_reads() {
    let server = Server {
        docs: HashMap::new(),
    };

    let text = "style grid {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"stroke") && labels.contains(&"font-color"));
    assert!(!labels.contains(&"shape"), "a grid is lines and numbers");
    assert!(!labels.contains(&"fill"), "{labels:?}");

    let text = "style title {\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"font-size"));
    assert!(!labels.contains(&"stroke"), "a heading is text");
}

/// The handful of names that are *not* free to choose. Without them
/// `style grid` looked like a typo and a typo looked like a theme.
#[test]
fn completion_offers_the_theme_names_after_style() {
    let server = Server {
        docs: HashMap::new(),
    };

    let text = "type rack\nstyle ";
    let items = server.items_for(text, "style ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    for name in ["device", "node", "edge", "group", "grid", "title"] {
        assert!(labels.contains(&name), "`{name}` missing from {labels:?}");
    }
    // The genre's own word comes first: it is the one that changes per file.
    assert_eq!(labels.first(), Some(&"device"));

    // And another genre offers its own word, not this one.
    let text = "type pie\nstyle ";
    let items = server.items_for(text, "style ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"slice") && !labels.contains(&"device"));
}

#[test]
fn completion_offers_extends_after_a_style_name() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "style warn ";
    let items = server.items_for(text, text, text.len());
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "extends");

    // Once, though — `extends parent ` is done asking.
    let text = "style base {\n}\nstyle warn extends base ";
    let items = server.items_for(text, "style warn extends base ", text.len());
    assert!(!items.iter().any(|i| i["label"] == "extends"));
}

/// Asking on a line holding nothing but indentation is exactly when you
/// most want the list, and it used to answer nothing.
#[test]
fn completion_answers_on_a_blank_indented_line() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "type network\n  ";
    let items = server.items_for(text, "  ", text.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"host"));
}

#[test]
fn completion_offers_enum_values_after_a_colon() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "node a {\n  shape: ";
    let items = server.items_for(text, "  shape: ", text.len());
    assert!(items.iter().any(|i| i["label"] == "cylinder"));
}

#[test]
fn completion_offers_keywords_at_statement_level() {
    let server = Server {
        docs: HashMap::new(),
    };
    let items = server.items_for("", "", 0);
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"node"));
    assert!(labels.contains(&"style"));
    assert!(labels.contains(&"graph"));
}

#[test]
fn hover_on_a_node_reports_its_resolved_geometry() {
    let text = "node a \"A\" at (10, 20) { size: 100 50 }\nnode b right-of a gap 30\n";

    let hover = hover::describe_node(text, "b").expect("b is declared");
    assert!(hover.contains("**node** `b`"), "{hover}");
    // a ends at x=110, plus a 30 gap.
    assert!(hover.contains("x 140"), "{hover}");
}

#[test]
fn hover_on_a_node_names_its_scoped_layout_relation() {
    let text = "node a \"A\" { size: 100 50 }\nnode b \"B\"\nlayout {\n  place a at (10, 20)\n  place b right-of a gap 30\n}\n";

    let hover = hover::describe_node(text, "b").expect("b is declared");
    assert!(hover.contains("**placement**"), "{hover}");
    assert!(hover.contains("right-of a  — declared"), "{hover}");
    assert!(hover.contains("gap 30  — declared"), "{hover}");
}

#[test]
fn outline_includes_groups_and_scoped_layouts() {
    let source = "group services \"Services\" {\n  node api \"API\"\n  node db \"DB\"\n  layout {\n    place api at (0, 0)\n    place db right-of api\n  }\n}\n";
    let symbols = server(source).symbols(&json!({
        "textDocument": { "uri": "file:///t.bd" }
    }));
    let entries = symbols.as_array().expect("a document outline");
    let group = entries
        .iter()
        .find(|symbol| symbol["name"] == "services")
        .expect("the group is visible");
    assert_eq!(group["detail"], "Services");
    let layout = entries
        .iter()
        .find(|symbol| symbol["name"] == "layout")
        .expect("the scoped block is visible");
    assert_eq!(layout["detail"], "in group services · 2 relations");
}

#[test]
fn hover_on_a_style_lists_its_properties() {
    let text = "style base { radius: 4 }\nstyle leaf extends base {\n  shape: cylinder\n}\n";

    let hover = hover::describe_style(text, "leaf").expect("leaf is declared");
    assert!(hover.contains("extends `base`"), "{hover}");
    assert!(hover.contains("shape: cylinder"), "{hover}");
}

#[test]
fn a_template_squiggles_while_it_is_written() {
    // A line repeating over two lists used to be a surprise at render
    // time; nothing was running the template's own rules in the editor.
    let source = "type pie\nslice {{a.id}} \"{{b.name}}\" 1\n";
    let found = diagnostics(source);
    assert!(
        found.iter().any(|d| d["message"]
            .as_str()
            .unwrap_or_default()
            .contains("repeats over")),
        "{found:?}"
    );
    // And the span is the line, not the whole file.
    let d = found
        .iter()
        .find(|d| {
            d["message"]
                .as_str()
                .unwrap_or_default()
                .contains("repeats over")
        })
        .unwrap();
    assert_eq!(d["range"]["start"]["line"].as_u64().unwrap(), 1);
}

#[test]
fn completion_inside_a_hole_offers_the_holes_already_used() {
    // A template is written by repeating its own parameter names, and
    // misspelling one silently produces a parameter nobody binds.
    let source = "type pie { title: \"{{quarter}}\" }\nslice {{r.id}} \"x\" 1\nslice {{";
    let items = server(source).items_for(source, "slice {{", source.len());
    let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains(&"r.id"), "{labels:?}");
    assert!(labels.contains(&"quarter"), "{labels:?}");
}

#[test]
fn hover_on_a_hole_says_what_it_binds() {
    // `value` inside `{{r.value}}` is not the pie property, and answering
    // with that documentation would be worse than answering nothing.
    let source = "type pie\nslice {{r.value}} \"x\" 1";
    let on_hole = source.find("r.value").unwrap() + 2;
    let md = server(source).hover(&at(source, on_hole))["contents"]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    assert!(md.contains("**hole**"), "{md}");
    assert!(md.contains("once per row"), "{md}");
    assert!(!md.contains("slice"), "not the archetype's property: {md}");
}

#[test]
fn the_outline_lists_a_templates_parameters() {
    let source = "type pie { title: \"{{quarter}}\" }\nslice {{r.id}} \"{{r.name}}\" 1";
    let symbols = server(source).symbols(&json!({
        "textDocument": { "uri": "file:///t.bd" }
    }));
    let names: Vec<&str> = symbols
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str())
        .collect();
    assert!(names.contains(&"{{quarter}}"), "{names:?}");
    assert!(names.contains(&"{{r.id}}"), "{names:?}");
}

#[test]
fn signature_help_names_the_slot_you_are_typing() {
    // The idiomatic form is positional, and it was the one thing nothing
    // could tell you the shape of while you were writing it.
    let source = "type packet\nfield sport \"Source port\" 0 ";
    let help = server(source).signature(&at(source, source.len()));

    let sig = &help["signatures"][0];
    assert_eq!(
        sig["label"].as_str().unwrap(),
        "field <id> [\"label\"] <bits> <bits>"
    );
    // Two values written after the label, so the cursor is on the second.
    assert_eq!(help["activeParameter"].as_u64().unwrap(), 3);

    // Nothing to say where there is no positional shape.
    let plain = "type class\nclass a \"A\" ";
    assert!(server(plain).signature(&at(plain, plain.len())).is_null());
}

#[test]
fn every_colour_gets_a_swatch() {
    // `#` is only ever a colour (`D-004`), which is what makes a lexical
    // scan of the file safe.
    let source =
        "graph { background: #0b1120 }\nnode a { fill: #38bdf880 }\n// #not-a-color-here\n";
    let colours = server(source).colours(&json!({
        "textDocument": { "uri": "file:///t.bd" }
    }));
    let list = colours.as_array().unwrap();
    // Two colours. `#not-a-color-here` has no hex digits after the `#`
    // and yields nothing, which is what keeps the scan safe.
    assert_eq!(list.len(), 2, "{colours}");

    // The alpha channel is read, not assumed.
    assert_eq!(list[1]["color"]["alpha"].as_f64().unwrap(), 128.0 / 255.0);
    assert_eq!(list[0]["color"]["alpha"].as_f64().unwrap(), 1.0);
}

#[test]
fn a_colour_comes_back_as_the_shortest_hex_that_says_it() {
    // Alpha defaults to opaque (`D-016`), so an opaque colour does not
    // need to write it.
    let opaque = colour_presentation(&json!({
        "color": { "red": 0.0, "green": 0.0, "blue": 1.0, "alpha": 1.0 }
    }));
    assert_eq!(opaque[0]["label"].as_str().unwrap(), "#0000ff");

    let translucent = colour_presentation(&json!({
        "color": { "red": 1.0, "green": 1.0, "blue": 1.0, "alpha": 0.5 }
    }));
    assert_eq!(translucent[0]["label"].as_str().unwrap(), "#ffffff80");
}

#[test]
fn highlighting_an_id_finds_it_everywhere_and_only_whole() {
    // `a` must not light up inside `api`.
    let source = "node a \"A\"\nnode api \"API\"\na -> api\n";
    let hits = server(source).highlights(&at(source, 5));
    assert_eq!(hits.as_array().unwrap().len(), 2, "{hits}");
}

#[test]
fn hover_on_an_archetypes_own_word_gives_its_form() {
    // The word you are least likely to be sure about, and it said nothing.
    let source = "type pie\nslice a \"A\" 10";
    let md = server(source).hover(&at(source, 10))["contents"]["value"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(md.contains("slice <id> [\"label\"] <value>"), "{md}");
    assert!(md.contains("type pie"), "{md}");
}

#[test]
fn hover_on_a_node_shows_the_paint_the_cascade_settled_on() {
    // Four layers deep, and the answer used to be visible only by
    // rendering the file.
    let source = "style node { fill: #123456 }\nnode a \"A\" { stroke: #abcdef 2 }";
    let on_a = source.find("node a").unwrap() + 5;
    let md = server(source).hover(&at(source, on_a))["contents"]["value"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(md.contains("#123456"), "the inherited fill: {md}");
    assert!(md.contains("#abcdef"), "the declared stroke: {md}");
}

#[test]
fn a_completion_writes_the_whole_statement() {
    // A list of words leaves you to type the rest from memory; a list of
    // statements does not.
    let items = server("type pie\n").items_for("type pie\n", "", 9);
    let slice = items
        .iter()
        .find(|i| i["label"] == "slice")
        .unwrap_or_else(|| panic!("{items:?}"));
    assert_eq!(slice["insertTextFormat"].as_u64().unwrap(), 2, "a snippet");
    assert!(
        slice["insertText"]
            .as_str()
            .unwrap()
            .starts_with("slice ${1:id} \"${2:label}\""),
        "{slice}"
    );
    assert!(
        slice["insertText"].as_str().unwrap().contains("value"),
        "the slot is named in the snippet: {slice}"
    );
}

#[test]
fn hover_on_a_property_key_documents_it() {
    let server = Server {
        docs: HashMap::new(),
    };
    let text = "node a { shape: rect }";
    let params = json!({
        "textDocument": { "uri": "file:///t.bd" },
        "position": { "line": 0, "character": 10 },
    });

    let mut server = server;
    server.docs.insert("file:///t.bd".into(), text.into());
    let hover = server.hover(&params);
    let value = hover["contents"]["value"].as_str().unwrap_or_default();
    assert!(value.contains("Outline geometry"), "{value}");
}

fn server_with(text: &str) -> Server {
    let mut server = Server::default();
    server.docs.insert("file:///t.bd".into(), text.into());
    server
}

fn labels_at(text: &str, line: &str) -> Vec<String> {
    server_with(text)
        .items_for(text, line, text.len())
        .iter()
        .filter_map(|i| i["label"].as_str().map(String::from))
        .collect()
}

#[test]
fn completion_stays_out_of_labels_and_comments() {
    // Typing prose is exactly when a popup is in the way.
    assert!(labels_at("node a \"Cli", "node a \"Cli").is_empty());
    assert!(labels_at("// note: sha", "// note: sha").is_empty());
    assert!(labels_at("node a { fill: #fff } // sh", "node a { fill: #fff } // sh").is_empty());
}

#[test]
fn a_dot_only_opens_styles_where_a_class_can_go() {
    let text = "style dark { fill: #000 }\n";

    // A decimal is not a class reference.
    let line = "node a { opacity: 0.";
    assert!(labels_at(&format!("{text}{line}"), line).is_empty());

    // Neither is a dotted arrow.
    let line = "a ..";
    assert!(!labels_at(&format!("{text}{line}"), line).contains(&"dark".to_string()));

    let line = "node a .";
    assert!(labels_at(&format!("{text}{line}"), line).contains(&"dark".to_string()));
}

#[test]
fn aliases_are_hidden_from_completion() {
    let labels = labels_at("node a {\n  ", "  ");
    assert!(labels.contains(&"stroke".to_string()));
    assert!(!labels.contains(&"border".to_string()), "{labels:?}");
    assert!(!labels.contains(&"color".to_string()));
    assert!(!labels.contains(&"text-color".to_string()));
}

#[test]
fn statement_level_stops_suggesting_once_a_word_is_finished() {
    // Trailing space used to re-open the keyword list on every word.
    assert!(
        labels_at("node alpha ", "node alpha ")
            .iter()
            .all(|l| l != "graph")
    );
    assert!(labels_at("n", "n").contains(&"node".to_string()));
}

#[test]
fn archetype_names_complete_after_type() {
    let labels = labels_at("type ", "type ");
    assert!(labels.contains(&"pie".to_string()));
    assert!(labels.contains(&"packet".to_string()));
    assert!(labels.contains(&"free".to_string()));
}

#[test]
fn a_type_block_completes_that_archetypes_settings() {
    let text = "type pie {\n  ";
    let labels = labels_at(text, "  ");
    assert!(labels.contains(&"donut".to_string()));
    assert!(!labels.contains(&"padding".to_string()), "{labels:?}");

    let text = "type packet {\n  ";
    let labels = labels_at(text, "  ");
    assert!(labels.contains(&"ruler".to_string()));
    assert!(!labels.contains(&"donut".to_string()));
}

#[test]
fn chart_data_is_offered_only_where_it_is_read() {
    let labels = labels_at("type pie\nnode a {\n  ", "  ");
    assert!(labels.contains(&"value".to_string()));
    assert!(!labels.contains(&"bits".to_string()));

    let labels = labels_at("type packet\nnode a {\n  ", "  ");
    assert!(labels.contains(&"bits".to_string()));
    assert!(!labels.contains(&"value".to_string()));

    let labels = labels_at("node a {\n  ", "  ");
    assert!(!labels.contains(&"value".to_string()));
    assert!(!labels.contains(&"bits".to_string()));
}

#[test]
fn placements_are_not_offered_where_placement_is_ignored() {
    assert!(labels_at("node a ", "node a ").contains(&"right-of".to_string()));
    assert!(!labels_at("type pie\nnode a ", "node a ").contains(&"right-of".to_string()));
}

#[test]
fn declared_archetype_defaults_to_free() {
    assert_eq!(declared_archetype("node a\n"), "free");
    assert_eq!(declared_archetype("type packet\nnode a\n"), "packet");
    assert_eq!(declared_archetype("type pie {\n}\n"), "pie");
    // `typed` is not `type`.
    assert_eq!(declared_archetype("node typed\n"), "free");
}

#[test]
fn hover_documents_an_archetype() {
    let text = "type packet";
    let server = server_with(text);
    let params = json!({
        "textDocument": { "uri": "file:///t.bd" },
        "position": { "line": 0, "character": 7 },
    });
    let hover = server.hover(&params);
    let value = hover["contents"]["value"].as_str().unwrap_or_default();
    assert!(value.contains("**type** `packet`"), "{value}");
}

#[test]
fn completion_offers_the_archetypes_own_keyword() {
    let labels = labels_at("type pie\ns", "s");
    assert_eq!(
        labels.first().map(String::as_str),
        Some("slice"),
        "{labels:?}"
    );
    assert!(labels.contains(&"node".to_string()), "node stays available");

    let labels = labels_at("type packet\nf", "f");
    assert_eq!(labels.first().map(String::as_str), Some("field"));

    // `free` has no second word to offer.
    let labels = labels_at("n", "n");
    assert_eq!(
        labels.iter().filter(|l| *l == "node").count(),
        1,
        "{labels:?}"
    );
}

#[test]
fn references_are_found_under_either_keyword() {
    // In an archetype with its own word, both spellings declare items, so
    // both must be offered as references.
    let text = "type pie\nslice alpha \"A\" 1\nnode beta \"B\" { value: 2 }\n";
    let found: Vec<String> = declared_items(text, "pie")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert!(found.contains(&"alpha".to_string()), "{found:?}");
    assert!(found.contains(&"beta".to_string()), "{found:?}");
}

#[cfg(feature = "bpmn")]
#[test]
fn every_word_of_a_genre_declares_something_the_editor_can_see() {
    // An archetype may have more than one word (`D-023`), and only the
    // first was being looked for — so in `xy` a `scatter` and in `bpmn` an
    // `event`, a `gateway` and a `data` were invisible to completion and
    // to go-to-definition alike. Four words is what made it obvious.
    let text = "type bpmn\ntask t \"T\"\nevent e \"E\" start\n\
                    gateway g \"G\" exclusive\ndata d \"D\" object\ngroup lane \"Lane\" { }\n";
    let found: Vec<String> = declared_items(text, "bpmn")
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    for want in ["t", "e", "g", "d", "lane"] {
        assert!(found.contains(&want.to_string()), "{want}: {found:?}");
    }
}

#[cfg(feature = "bpmn")]
#[test]
fn go_to_definition_finds_a_box_declared_under_its_genres_own_word() {
    // It looked for `node` and `style` and nothing else, so in every genre
    // with a word of its own — twenty-eight of twenty-nine — the jump
    // silently did nothing. The failure was invisible because returning
    // `null` is also what "no such name" looks like.
    let text = "type bpmn\ntask pay \"Pay\" service\n\
                    event late \"Late\" boundary timer { attach: pay }\n";
    let server = server_with(text);
    // The cursor sits on `pay` inside `attach: pay`, the last occurrence.
    let at = text.rfind("pay").expect("the reference is there");
    let (line, col) = LineIndex::new(text).line_col(at + 1);
    let answer = server.definition(&json!({
        "textDocument": { "uri": "file:///t.bd" },
        "position": { "line": line, "character": col },
    }));

    assert_eq!(
        answer["range"]["start"]["line"].as_u64(),
        Some(1),
        "expected the `task pay` line, got {answer}"
    );
}

#[test]
fn placements_follow_the_archetypes_keyword() {
    assert!(labels_at("node a ", "node a ").contains(&"right-of".to_string()));
    // A pie ignores placement, so it is not offered — with either word.
    assert!(!labels_at("type pie\nslice a ", "slice a ").contains(&"right-of".to_string()));
}
