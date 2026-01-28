# Documentation Colony

Ce dossier centralise la documentation du projet Colony (architecture, configuration, conventions, décisions, etc.).

## Contenu attendu

- Architecture générale et principes.
- Convention de configuration (structure, schémas, exemples).
- Décisions techniques majeures (ADR ou notes).
- Guides d’intégration d’applications.

## Priorités techniques

- Se focaliser sur la **réactivité** et une faible consommation **CPU/RAM/disque**.
- Prioriser l’**asynchrone** pour éviter de surcharger Colony.
- Activer les modules **uniquement lors de leur utilisation**.
- Réaliser les appels réseau **via API**.

## Décisions récentes

- **UI** : Iced est adopté pour l’interface graphique.
- **Configuration** : `config/colony.toml` définit les dossiers à scanner et les paramètres de scan.

## À maintenir

Ce fichier doit être mis à jour à chaque évolution majeure afin de garder une vision claire et fiable du projet.
