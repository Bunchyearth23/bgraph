"""Expose canonical manuals and the existing gallery without keeping copies."""

from pathlib import Path

from mkdocs.structure.files import File


ROOT = Path(__file__).resolve().parent.parent


def on_files(files, config, **kwargs):
    for name in ("reference.md", "language.md", "API.md"):
        files.append(File.generated(config, name, abs_src_path=str(ROOT / "docs" / name)))

    gallery = [
        "# Gallery of all 30 types\n",
        "Each preview and its `.bd` source come from the repository gallery. "
        "Download a source file, then run `bgraph watch example.bd --open --port 0` "
        "to preview it while editing. You can also create each example with "
        "`bgraph init <type> -o example.bd`.\n",
        "[How do I choose a type?](graphiques.md) · [Your first diagram](demarrage.md)\n",
    ]
    for source in sorted((ROOT / "examples").glob("*.bd")):
        image = source.with_suffix(".svg")
        if not image.is_file():
            raise FileNotFoundError(f"Missing gallery image: {image}")
        for asset in (source, image):
            files.append(File.generated(config, f"examples/{asset.name}", abs_src_path=str(asset)))
        gallery.append(
            f"## {source.stem}\n\n"
            f"![{source.stem} example](examples/{image.name}){{ loading=lazy }}\n\n"
            f"[Download .bd source](examples/{source.name}) · [View SVG](examples/{image.name})\n"
        )
    files.append(File.generated(config, "galerie.md", content="\n".join(gallery)))
    return files


def on_page_markdown(markdown, page, **kwargs):
    if page.file.src_uri not in ("reference.md", "language.md"):
        return markdown
    # These manuals also render on GitHub, where their repository links differ.
    links = {
        "[`../README.md`](../README.md)": "[Home](index.md)",
        "(guide/demarrage.md)": "(demarrage.md)",
        "(guide/editeurs.md)": "(editeurs.md)",
        "(../editors/zed)": "(editeurs.md#zed)",
        "(../editors/vscode)": "(editeurs.md#vs-code)",
        "(../editors/README.md)": "(editeurs.md#other-editors)",
        "(../src/syntax.pest)": "(https://github.com/Bunchyearth23/bgraph/blob/main/src/syntax.pest)",
        "(../tree-sitter-bgraph)": "(https://github.com/Bunchyearth23/bgraph/tree/main/tree-sitter-bgraph)",
    }
    for old, new in links.items():
        markdown = markdown.replace(old, new)
    return markdown
