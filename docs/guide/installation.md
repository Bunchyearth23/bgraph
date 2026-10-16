# Installer bgraph

## À partir d'un paquet fourni

Si vous avez reçu une archive ou un dossier d'export bgraph :

1. Décompressez le paquet correspondant à votre système et à votre architecture.
2. Placez `bgraph` et `bgraph-lsp` dans un dossier de votre `PATH`
   (`bgraph.exe` et `bgraph-lsp.exe` sous Windows).
3. Ouvrez un nouveau terminal et lancez :

   ```sh
   bgraph --version
   bgraph --help
   ```

Le CLI génère les images. Le serveur `bgraph-lsp` fournit les diagnostics,
la complétion et la navigation aux éditeurs. Consultez le fichier `LICENSE`
livré avec le paquet pour les conditions d'utilisation.

## Depuis le dépôt

Cette méthode suppose que vous avez accès aux sources et que Rust est installé.
Le projet demande **Rust 1.88 ou plus récent**.
Depuis la racine du dépôt :

```sh
cargo install --locked --path . --bins
```

Les exécutables sont installés dans le dossier Cargo des binaires,
habituellement `~/.cargo/bin` ou `%USERPROFILE%\.cargo\bin`.
Ce dossier doit être dans le `PATH`.

Pour travailler sans installer les binaires :

```sh
cargo build --locked --release
./target/release/bgraph --version
```

Sous PowerShell, le chemin équivalent est `.\target\release\bgraph.exe`.
Les commandes des guides utilisent `bgraph` ; remplacez-le par ce chemin
si vous utilisez une compilation locale.

## Activer l'export PNG

SVG et PDF sont disponibles dans la compilation par défaut. Le PNG est
optionnel ; pour l'ajouter lors de l'installation :

```sh
cargo install --locked --path . --bins --features png --force
```

Ou, sans installation :

```sh
cargo build --locked --release --features png
```

## Installer l'éditeur

Les extensions sont préparées séparément : voir
[Zed et VS Code](editeurs.md). Une installation du CLI suffit pour commencer
avec n'importe quel éditeur de texte et la preview navigateur.
