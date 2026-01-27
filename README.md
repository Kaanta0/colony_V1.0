# Colony

Colony est un launcher pour l’écosystème d’applications du projet **Colony**. L’objectif est de fournir une base claire, flexible et évolutive pour orchestrer toutes les apps du projet depuis une seule interface.

## Objectifs

- Centraliser le lancement et la gestion des applications Colony.
- Éviter le **hardcodage** : tout doit être configurable (dossiers, colonnes, sections, scripts, etc.).
- Préparer une architecture modulable pour des évolutions futures (UI, scripts, plugins, etc.).

## Stack technique

- **Langage** : Rust
- **Build** : Cargo (`Cargo.toml` + `Cargo.lock`)
- **UI** : `slint`

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

La configuration par défaut est dans `config/colony.toml`. Vous pouvez pointer un autre fichier
en définissant la variable d’environnement `COLONY_CONFIG`.

Les instructions de build/exécution seront ajoutées une fois la base Rust en place.

---

Si vous souhaitez proposer des conventions supplémentaires, ouvrez une issue ou une PR.
