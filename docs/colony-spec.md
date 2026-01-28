# Spécification — Découverte de logiciels Colony via GitHub

## Contexte
Colony est un launcher d’applications destiné à télécharger, mettre à jour et lancer les logiciels de l’écosystème Colony. Les logiciels sont classés dans des **catégories internes** à Colony, indépendantes des plateformes (Windows/Linux).

## Source officielle
- **Compte GitHub officiel :** `MotherSphere` (compte utilisateur, pas une organisation).
- Colony scanne automatiquement **tous les dépôts GitHub** du compte MotherSphere pour détecter des logiciels compatibles.

## Détection d’un logiciel Colony
Un dépôt est reconnu comme un logiciel Colony **uniquement si** un fichier manifeste nommé **`colony.json`** est présent **à la racine** du dépôt.

Le manifeste sert à :
- identifier le logiciel,
- définir les plateformes compatibles (Windows / Linux),
- indiquer quels fichiers télécharger depuis les releases GitHub,
- permettre l’affichage du logiciel dans l’interface Colony.

## Versioning & mise à jour
- Colony récupère les versions via les **tags GitHub** au format `vX.Y.Z`.
- Colony télécharge automatiquement la **dernière release compatible** avec le système de l’utilisateur.
- Comparaison **version locale vs version distante** :
  - si une version plus récente existe → **proposition de mise à jour**,
  - sinon → **lancement direct** du logiciel.

## Enrichissement automatique (métadonnées GitHub)
Colony peut extraire des informations depuis le dépôt pour enrichir l’affichage :

- **Titre** : par défaut le nom du dépôt (ex. `orCAL`).
- **Description** : contenu du README (ou fallback sur la description GitHub si nécessaire).
- **Langage principal** : via l’API GitHub (langage dominant du dépôt).

### Exemple
Dépôt : `https://github.com/MotherSphere/orCAL`
- `colony.json` est détecté à la racine.
- Titre affiché : `orCAL`.
- Description : extrait du README.
- Langage : principal du dépôt.

## Recommandations de stabilité / performance
- Utiliser le cache HTTP (ETag / If-None-Match) pour limiter les appels GitHub.
- Limiter la taille de la description (README) affichée dans Colony.
- Prévoir des **fallbacks** si README ou langage indisponibles.

