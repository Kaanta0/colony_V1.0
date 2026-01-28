# Colony

Colony est un launcher pour l’écosystème d’applications du projet **Colony**. L’objectif est de fournir une base claire, flexible et évolutive pour orchestrer toutes les apps du projet depuis une seule interface.

## Objectifs

- Centraliser le lancement et la gestion des applications Colony.
- Éviter le **hardcodage** : tout doit être configurable (dossiers, colonnes, sections, scripts, etc.).
- Préparer une architecture modulable pour des évolutions futures (UI, scripts, plugins, etc.).

## Stack technique

- **Langage** : Rust
- **Build** : Cargo (`Cargo.toml` + `Cargo.lock`)
- **UI** : `iced`

## Obligations à respecter (permanent)

- Utiliser **uniquement les dernières versions** de Rust et des librairies/dépendances.
- Minimiser au maximum le **hardcodage** : tout ce qui est structurel doit être défini via configuration.
- Prioriser la **réactivité** et les usages **CPU/RAM/disque** faibles.
- Favoriser l’**asynchrone** pour éviter de surcharger Colony.
- Activer les modules **uniquement au moment de leur utilisation**.
- Effectuer les appels réseau **via API**.

## Structure du dépôt

```
./
├── README.md
├── docs/
│   └── docs.md
└── tasks/
    └── tasks.md
```

## Documentation et suivi

- Le dossier `docs/` contient la documentation globale du projet.
- Le dossier `tasks/` sert à suivre les tâches, décisions et actions à venir.
- Chaque dossier dispose d’un fichier `.md` d’orientation.

## Démarrage

```bash
cargo run
```

## Assets UI (polices)

Les polices Nerd Font utilisées par l’interface se trouvent dans
`src/ui/assets/fonts/JetBrainsMonoNerdFont/` (notamment les variantes
`Regular`, `Medium` et `Bold`). L’application tente de charger ces fichiers au
démarrage. Si les assets sont absents ou illisibles, Colony bascule sur une
police système monospace en fallback.

### Configuration des sections

Les sections affichées dans la sidebar sont définies dans `config/categories.json`. Chaque entrée
décrit le nom, l’icône et le filtre à appliquer :

```json
[
  { "name": "All", "icon": "\uf00a", "origin": "non_windows", "category": "all" },
  { "name": "Windows", "icon": "\uf17a", "origin": "windows", "category": "all" }
]
```

- `name` : libellé affiché.
- `icon` : caractère unicode (icônes Nerd Font).
- `origin` : filtre d’origine (`any`, `windows`, `non_windows`).
- `category` : filtre de catégorie (`development`, `graphics`, `network`, `office`, `multimedia`,
  `system`, `utility`, `game`, `other`, ou `all`).

Si le fichier est absent ou invalide, l’application retombe sur les sections par défaut
codées en interne.

### Configuration des dossiers de scan

Les dossiers scannés pour détecter les applications sont définis dans `config/colony.toml` :

```toml
[scan]
windows = [
  "${ProgramData}\\Microsoft\\Windows\\Start Menu\\Programs",
  "${APPDATA}\\Microsoft\\Windows\\Start Menu\\Programs",
]

unix = [
  "${HOME}/.local/share/applications",
  "/usr/share/applications",
  "/usr/local/share/applications",
  "/var/lib/flatpak/exports/share/applications",
  "${HOME}/.local/share/flatpak/exports/share/applications",
  "/var/lib/snapd/desktop/applications",
]
```

- `windows` : liste explicite des dossiers à scanner sous Windows.
- `unix` : liste explicite des dossiers à scanner sous Linux/macOS.
- Les variables d’environnement `${...}` (et `%...%` côté Windows) sont résolues à l’exécution.

Si le fichier est absent ou invalide, Colony utilise les valeurs historiques (Start Menu Windows,
`$HOME/.local/share/applications`, `$XDG_DATA_DIRS` ou `/usr/share/applications`, etc.).

Les instructions de build/exécution seront ajoutées une fois la base Rust en place.

---

Si vous souhaitez proposer des conventions supplémentaires, ouvrez une issue ou une PR.
