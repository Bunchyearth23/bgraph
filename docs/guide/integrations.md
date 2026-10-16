# Rust, navigateur et Typst

## Utiliser bgraph depuis Rust

Le projet n'est pas publié sur crates.io. Avec un accès au dépôt, ajoutez
une dépendance de chemin dans votre `Cargo.toml` :

```toml
[dependencies]
bgraph = { path = "/chemin/vers/bgraph", default-features = false }
```

Sans features supplémentaires, la bibliothèque conserve le genre `free`.
Activez les genres dont vous avez besoin, par exemple
`features = ["pie", "packet"]`, ou `all-archetypes` pour l'ensemble.
La feature `pdf` ajoute les fonctions de sortie PDF.

```rust
fn main() {
    let source = r#"
        node a "Départ"
        node b "Arrivée"
        layout { place b right-of a gap 48 }
        a -> b
    "#;
    let svg = bgraph::render_source(source).expect("diagramme valide");
    std::fs::write("diagramme.svg", svg).expect("écriture du SVG");
}
```

`render_source` prend du texte autonome et ne lit pas de fichiers.
Pour résoudre des inclusions, utilisez :

```rust
let svg = bgraph::render_file(std::path::Path::new("architecture.bd"))?;
```

La [page API](API.md) précise les fonctions stables, les diagnostics et les
surfaces d'inspection qui ne font pas partie de la promesse de compatibilité.

## Essayer le playground dans un navigateur

Depuis le dépôt, avec Rust, Node et pnpm :

```sh
rustup target add wasm32-unknown-unknown
./playground/build.sh
cd playground
pnpm install
pnpm dev
```

Ouvrez l'adresse annoncée par Vite. Vous disposez d'un éditeur de source,
du rendu SVG et des diagnostics. Le menu des genres propose les exemples
du compilateur.

`include` et `from:` ne sont pas disponibles dans ce contexte : la page
ne possède pas de répertoire source sur disque. Pour ces usages,
générez le SVG avec le CLI.

## Insérer une image dans Typst

La méthode la plus directe consiste à générer un SVG :

```sh
bgraph architecture.bd -o architecture.svg
```

Dans le document Typst, dans le même dossier que l'image :

```typst
#image("architecture.svg", width: 80%)
```

## Générer pendant la compilation Typst

Pour utiliser le plugin local, préparez son module depuis le dépôt :

```sh
rustup target add wasm32-unknown-unknown
./typst/build.sh
```

Dans un document situé à la racine du dépôt :

```typst
#import "typst/bgraph.typ": bg-diagram-file

#bg-diagram-file(path("architecture.bd"), width: 80%)
```

Adaptez le chemin de l'import si votre document est ailleurs.
Le `path("architecture.bd")` doit être créé dans le document appelant pour
conserver son répertoire. Un diagramme invalide fait échouer la compilation
Typst avec le message du compilateur bgraph.

Le plugin attend un source autonome. Pour des inclusions ou du CSV externe,
utilisez la génération SVG préalable.
