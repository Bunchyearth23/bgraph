"""Expose canonical manuals and the existing gallery without keeping copies."""

from pathlib import Path

from mkdocs.structure.files import File


ROOT = Path(__file__).resolve().parent.parent


def on_files(files, config, **kwargs):
    for name in ("reference.md", "language.md", "API.md"):
        files.append(File.generated(config, name, abs_src_path=str(ROOT / "docs" / name)))

    gallery = [
        "# Galerie des 30 types\n",
        "Chaque aperçu et son fichier `.bd` proviennent de la galerie du dépôt. "
        "Téléchargez une source, puis lancez `bgraph watch fichier.bd` pour la modifier. "
        "Vous pouvez aussi recréer chaque exemple avec `bgraph init <type>`.\n",
        "[Comment choisir un type ?](graphiques.md)\n",
    ]
    for source in sorted((ROOT / "examples").glob("*.bd")):
        image = source.with_suffix(".svg")
        if not image.is_file():
            raise FileNotFoundError(f"Missing gallery image: {image}")
        for asset in (source, image):
            files.append(File.generated(config, f"examples/{asset.name}", abs_src_path=str(asset)))
        gallery.append(
            f"## {source.stem}\n\n"
            f"![Exemple {source.stem}](examples/{image.name}){{ loading=lazy }}\n\n"
            f"[Source .bd](examples/{source.name}) · [Image SVG](examples/{image.name})\n"
        )
    files.append(File.generated(config, "galerie.md", content="\n".join(gallery)))
    return files


def on_page_markdown(markdown, page, **kwargs):
    if page.file.src_uri not in ("reference.md", "language.md"):
        return markdown
    # These manuals also render on GitHub, where their repository links differ.
    links = {
        "[`../README.md`](../README.md)": "[accueil](index.md)",
        "(../editors/zed)": "(editeurs.md#zed)",
        "(../editors/vscode)": "(editeurs.md#vs-code)",
        "(../editors/README.md)": "(editeurs.md#autres-editeurs)",
        "(../src/syntax.pest)": "(https://github.com/Bunchyearth23/bgraph/blob/main/src/syntax.pest)",
        "(../tree-sitter-bgraph)": "(https://github.com/Bunchyearth23/bgraph/tree/main/tree-sitter-bgraph)",
    }
    for old, new in links.items():
        markdown = markdown.replace(old, new)
    return markdown
