#![cfg(all(feature = "gitgraph", feature = "pdf"))]

#[test]
fn gitgraph_example_exports_its_tag_as_text_in_pdf_without_an_embedded_font() {
    let source = bgraph::init::skeleton("gitgraph").unwrap();
    let svg = bgraph::render_source(&source).unwrap();
    assert!(
        svg.contains("tag: v0.1") && svg.contains("tag: v0.2"),
        "{svg}"
    );
    assert!(!svg.contains('⌾'));
    let pdf = bgraph::render_source_pdf(&source).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let content = String::from_utf8_lossy(&pdf);
    assert!(
        content.contains("tag: v0.1") && content.contains("tag: v0.2"),
        "{content}"
    );
    assert!(!content.contains('?'));
}
