# Laboratoire NNUE : étapes restantes

Branche `feat/nnue-lab`. Un commit par étape, résultat montré avant de passer à la suivante.

## Où on en est

| Étape | Contenu | Commit |
| --- | --- | --- |
| 1 | JSON Schema des architectures, validation, presets | `9703336` |
| 2 | Format réseau v2, inférence générique, non-régression | `6b31f91` |
| 3 | Trait `Trainer` sur burn, jobs asynchrones persistants | `02f5710` |
| 4 | API REST `/api/v1`, OpenAPI 3.1, `/docs`, workers HTTP | `30d675a` |
| 5 | SDK Python/TypeScript, exemples en 4 langages, CI de synchronisation | `19d9cb2` |
| 6 | Évaluations SPRT dans l'arène, comparaison de modèles | `c4070f5` |
| **7** | **Protocole de trainer externe + exemple Python + docs** | à faire |
| **8** | **Front (éditeur, suivi, modèles) + sélecteur dans l'analyse Chess.com** | à faire |
| **9** | **Optimisation SIMD des architectures courantes + benchmark NPS** | à faire |

## Étape 7 : protocole de trainer externe (B3, B5, B7)

Objectif : pouvoir écrire un backend d'entraînement dans un autre langage, par exemple Python/PyTorch, derrière le trait `Trainer`.

- **Protocole JSON lines** sur stdin/stdout (choisi plutôt que gRPC). Il couvre :
  - la configuration du run ;
  - la progression ;
  - les checkpoints ;
  - l'export du réseau au format v2.
- **Côté Rust** : un backend `external` du trait `Trainer`, qui lance le processus et lui parle selon le protocole. Il passe par un pont `chess-train external-host`.
- **Exemple Python minimal** du protocole, dans un conteneur isolé `trainer-python`. Le brief exige que les backends externes tournent dans un conteneur isolé.
- **Docker** : un service `trainer-python` dans `docker-compose.yml`. Le serveur doit déclarer le nom de ce trainer pour que les jobs puissent le demander (`"trainer"` dans la requête d'entraînement).
- **Tests** : le protocole testé avec le backend Python d'exemple, de bout en bout (job → réseau v2 accepté par le lab).
- **Documentation** :
  - `docs/trainer-protocol.md` : le contrat complet, avec l'exemple Python minimal.
  - `docs/nnue-api.md` : concepts, schéma d'architecture commenté champ par champ, cycle de vie d'un modèle (entraîner → évaluer → activer), authentification, erreurs, limites. Il doit aussi couvrir les évaluations de l'étape 6.
  - Les exemples de la doc doivent être testés, pour rester synchronisés avec le code.
- **README** et docker-compose mis à jour.

## Étape 8 : front (B6, B8, B9)

Le front doit utiliser le **SDK TypeScript généré** (`sdk/typescript`).

**Accès à l'API**
- La clé API est saisie une fois dans l'interface et gardée dans le `localStorage`.
- `CHESS_API_AUTH=disabled` fonctionne en développement.

**Éditeur d'architecture**
- Formulaire et vue JSON synchronisés.
- Validation en direct (`/architectures:validate`), erreurs affichées au bon champ.
- Schéma visuel des couches.
- Estimation de la taille du réseau et du coût d'inférence.
- Départ possible depuis un preset.

**Page de suivi d'entraînement**
- Courbes de perte en direct (SSE `/training-runs/{id}/events`).
- Checkpoints.
- Boutons stop / resume.

**Page modèles**
- Liste avec provenance.
- Comparaison (`/evaluations:compare`) : taille, NPS, Elo.
- Résultats des matchs, et lancement d'une évaluation avec suivi en direct.
- Bouton « activer ».

**Migration de l'ancienne page Entraînement** (décision prise à l'étape 0)
- La page Entraînement est reconstruite sur `/api/v1`.
- Les anciennes routes `/api/training/*` sont supprimées.
- L'entraînement continu devient une source `live-selfplay`.
- L'arène, les bots et les tournois restent tels quels.

**Sélecteur de moteur dans l'analyse Chess.com (B9)**
- Choix entre Stockfish et mon moteur avec un modèle NNUE donné.
- Permet de comparer les classifications d'une même partie selon le modèle.
- **Aucune autre modification** de la fonctionnalité d'analyse.

## Étape 9 : optimisation SIMD + benchmark NPS (B2)

- Chemins SIMD rapides pour les formes courantes (presets), avec repli générique pour le reste. La sortie générique est encore scalaire.
- Tableau de NPS final, comparé au moteur d'avant le laboratoire.

Mesures actuelles (i7-9700KF, bench profondeur 13) :

| Réseau | NPS |
| --- | --- |
| Réseau legacy (chemin rapide) | 4,23 M (4,28 M avant le labo) |
| Réseau legacy, chemin générique forcé | 2,9 M |
| `fast-768` | ~1,0 M |
| `halfka-medium` | ~0,55 M |
| `multilayer` | ~0,29 M |

## Reste à faire en fin de branche

- **CI** : pousser la branche. Les jobs ajoutés aux étapes 5 et 6 (régénération des SDK, exemples de bout en bout, mêmes parties dans les 4 évaluations) n'ont encore jamais tourné sur GitHub.
- **Revue finale** des exigences du brief :
  - **tests** : validation de spec, round-trip, rejet de hash, non-régression, contrat API, protocole externe ;
  - **Docker GPU**, README, `.env` ;
  - **aucun secret en dur**.
- **Pull request** vers `main`.
- **Fichiers à ne jamais committer** :
  - `data/bots/main-sb305.nnue` : la copie de référence est dans `C:\Users\Nicolas\Documents\nnue-fixtures\` ;
  - les captures de `exemples bilan chess com/`.
