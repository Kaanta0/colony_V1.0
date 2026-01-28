# Documentation Colony

Ce dossier centralise la documentation du projet Colony (architecture, configuration, conventions, décisions, etc.).

## Contenu attendu

- Architecture générale et principes.
- Convention de configuration (structure, schémas, exemples).
- Décisions techniques majeures (ADR ou notes).
- Guides d’intégration d’applications.
- Schéma de manifestes Colony et exemples JSON.

## Priorités techniques

- Se focaliser sur la **réactivité** et une faible consommation **CPU/RAM/disque**.
- Prioriser l’**asynchrone** pour éviter de surcharger Colony.
- Activer les modules **uniquement lors de leur utilisation**.
- Réaliser les appels réseau **via API**.

## Décisions récentes

- **UI** : Iced est adopté pour l’interface graphique.
- **Configuration** : `config/colony.toml` définit les dossiers à scanner et les paramètres de scan.

## Configuration des scans

Le fichier `config/colony.toml` liste explicitement les dossiers à scanner par OS :

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

- Les variables `${...}` et `%...%` sont résolues à l’exécution.
- Si le fichier est absent ou invalide, Colony revient aux valeurs par défaut (Start Menu Windows,
  XDG data dirs, Flatpak, Snap, etc.).

## À maintenir

Ce fichier doit être mis à jour à chaque évolution majeure afin de garder une vision claire et fiable du projet.

## Manifeste Colony

Le fichier `colony.json` décrit une application Colony (identité, plateformes compatibles, téléchargements). Le
schéma est défini par `src/manifest.rs` et un exemple complet est disponible dans `docs/manifest_example.json`.
