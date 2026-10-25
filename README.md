# chess-bot

Moteur d'échecs en Rust avec trois parties :

1. **Moteur** : bitboards, recherche alpha-beta, compatible UCI.
2. **Entraînement** : self-play et entraînement d'un réseau NNUE avec [burn](https://burn.dev), sur GPU NVIDIA avec repli automatique sur le CPU.
3. **Visualisation** : interface web (React + TypeScript) pour jouer, analyser et suivre l'entraînement.

> **État : phase 4 sur 7.** Le moteur joue (perft exact, recherche alpha-beta, évaluation PeSTO, UCI complet) et l'interface web permet de jouer contre lui et d'analyser des parties en direct. L'entraînement NNUE arrive en phases 5 et 6. Voir [Feuille de route](#feuille-de-route).

## Démarrage rapide

```sh
docker compose up --build        # UI + API sur http://localhost:8080
just gpu-check                   # vérifie que le GPU est visible depuis Docker
```

## Prérequis

| OS | À installer |
| --- | --- |
| **Windows 10/11** | [Docker Desktop](https://www.docker.com/products/docker-desktop/) avec le backend **WSL2**. Pour le GPU : un pilote NVIDIA récent pour Windows. Ne pas installer de pilote dans WSL : Docker Desktop expose le GPU tout seul. |
| **Linux** | Docker Engine + plugin Compose. Pour le GPU : le pilote NVIDIA et le [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html) (`nvidia-ctk runtime configure --runtime=docker`). |
| **macOS** | Docker Desktop. Pas de CUDA : l'entraînement passe par `train-cpu`. |

Pour développer hors Docker : [rustup](https://rustup.rs) (la version est fixée par `rust-toolchain.toml`), [just](https://just.systems), et Node.js ≥ 22 pour le front.
Sous Windows : `winget install Rustlang.Rustup Casey.Just OpenJS.NodeJS.LTS`, plus les *Build Tools* MSVC.

**Performances sous Windows.** Les builds Docker et les lectures disque sont nettement plus rapides si le dépôt est cloné dans le système de fichiers WSL (`\\wsl$\Ubuntu\home\...`) plutôt que sur `C:\`. Si le dépôt reste sur `C:\`, les datasets (lus en continu pendant l'entraînement) sont de toute façon placés dans un volume Docker nommé, `datasets`, qui vit côté Linux.

**Permissions sous Linux.** Les conteneurs tournent sous un utilisateur non-root (UID 10001). Pour que les fichiers écrits dans `./data` vous appartiennent, lancez `export CHESS_UID=$(id -u) CHESS_GID=$(id -g)` avant `docker compose`.

## Commandes

`just` liste toutes les recettes. Les principales :

| Commande | Effet |
| --- | --- |
| `docker compose up` / `just up` | serveur + UI sur <http://localhost:8080> |
| `docker compose --profile train up` / `just train` | entraînement sur GPU (image CUDA) |
| `docker compose --profile train-cpu up` / `just train-cpu` | entraînement sur CPU (image sans CUDA) |
| `just gpu-check` | lance `nvidia-smi` dans un conteneur, puis un matmul et une rétropropagation burn sur le GPU, comparés au CPU |
| `just web-dev` | Vite (rechargement à chaud) + `chess-server` natif, sans reconstruire d'image |
| `just test` / `just lint` | tests et vérifications de la CI (rustfmt, clippy `-D warnings`, ESLint, tsc, Prettier) |
| `just perft` | perft profond en release (positions CPW + cas limites, ~760M nœuds, quelques secondes) |
| `just play` | moteur UCI sur stdin/stdout |
| `just engine-bench` | signature et vitesse (nœuds/s) du moteur, via `chess-uci bench` |
| `just gen-types` | régénère les types TypeScript depuis les structs Rust |

## Architecture

```
crates/
  chess-core/    plateau, bitboards, coups, FEN/PGN, Zobrist (aucune I/O, aucune dépendance projet)
  chess-eval/    trait Evaluator + évaluation classique + inférence NNUE (CPU/SIMD)
  chess-search/  alpha-beta générique sur Evaluator (ne voit que le trait)
  chess-uci/     binaire UCI
  chess-train/   binaire : self-play, entraînement burn (CUDA ou CPU), export des poids
  chess-server/  binaire : API REST (OpenAPI) + WebSocket (axum)
web/             React + TypeScript + Vite + Tailwind
docker/          Dockerfiles (engine, train, web) + nginx.conf
data/            poids, checkpoints, métriques (ignoré par git)
```

- **Règles de dépendance.** `chess-search` dépend de `chess-eval` avec `default-features = false` et ne voit donc que le trait `Evaluator`. La CI lance `cargo check -p chess-search` seul pour le garantir. Seuls les binaires font de l'I/O, du logging et de la configuration.
- **Types partagés.** Les structs de l'API dérivent `ts_rs::TS`. `cargo test` écrit leur équivalent TypeScript dans `web/src/api/generated/`, et la CI échoue si ces fichiers ne sont pas à jour.
- **Une seule origine.** Nginx (Docker) ou Vite (dev) sert l'UI et proxifie `/api` et `/ws` vers `chess-server`. Il n'y a donc pas de CORS.
- **GPU.** `chess-train` est compilé avec la feature `cuda` dans l'image GPU. Au démarrage, il cherche le pilote CUDA sans risquer de panic, puis utilise le GPU, ou le CPU (backend burn `flex`) s'il n'en trouve pas. Le device retenu est toujours affiché. cubecl compile les kernels lui-même (LLVM → PTX) : ni nvcc ni NVRTC ne sont nécessaires, et le build CUDA se fait dans l'image `nvidia/cuda:*-base`. L'image `devel` reste possible avec `--build-arg CUDA_BUILD_FLAVOR=devel`.

### Configuration de `chess-server`

Ordre de priorité : valeurs par défaut < fichier TOML (`chess-server.toml`, ou le chemin donné par `CHESS_SERVER_CONFIG`) < variables `CHESS_SERVER_*`.

```toml
bind = "0.0.0.0:3000"
data_dir = "data"
log_format = "pretty"   # ou "json"
max_searches = 8        # recherches simultanées (défaut : nombre de cœurs)
engine_hash_mb = 16     # table de transposition de chaque recherche
```

Le niveau de log se règle avec `RUST_LOG` (ex. `RUST_LOG=debug`).

## Interface web

<http://localhost:8080> (Docker) ou <http://localhost:5173> (`just web-dev`).

- **Jouer** : échiquier en glisser-déposer ou clic-clic, choix de la couleur (blancs, noirs, aléatoire), niveau réglable en profondeur (1 à 20) ou en temps par coup (0,1 à 10 s), annulation de coup, import/export FEN et PGN.
- **Analyse** : barre d'évaluation, score, ligne principale en SAN, flèche du meilleur coup, infos de recherche en direct (profondeur, nœuds, vitesse, remplissage de la table). L'analyse est relancée à chaque coup. On navigue dans la partie avec les flèches ← → (Origine et Fin pour aller au début ou à la fin), ou en cliquant dans la liste des coups.
- **Entraînement** : en phase 6.

Le front ne contient aucune règle d'échecs : chaque coup est validé par le moteur, qui renvoie aussi les coups légaux, le SAN et l'état de la partie.

### API

Documentation OpenAPI complète : `GET /api/openapi.json`.

L'API est **sans état** : une partie est toujours envoyée sous la forme `{ "fen": "<départ, optionnel>", "moves": ["e2e4", ...] }`, et le serveur la rejoue et la valide (erreur 422 avec un message en cas de coup illégal).

| Route | Rôle |
| --- | --- |
| `GET /api/health` | état et version du serveur |
| `POST /api/position` | rejoue une partie : FEN, coups légaux (UCI + SAN), échec, statut, historique SAN |
| `POST /api/engine/move` | coup du moteur, avec limites `depth`, `movetimeMs`, `nodes` (1 s par défaut) |
| `POST /api/pgn/import` | première partie d'un PGN |
| `POST /api/pgn/export` | partie au format PGN |
| `GET /ws` | WebSocket : `analyze` puis un message `info` par profondeur et un `done` final ; `stop` interrompt l'analyse |

Les scores renvoyés sont du point de vue des blancs. Les limites sont plafonnées côté serveur (profondeur 64, 60 s par coup, 10 min d'analyse), et le nombre de recherches simultanées est borné par `max_searches`.

## Utiliser le moteur dans une GUI (Cute Chess, Arena)

```sh
cargo build --release -p chess-uci
# binaire : target/release/chess-uci (chess-uci.exe sous Windows)
```

- **Cute Chess** : *Tools → Settings → Engines → +*. Mettre le chemin du binaire dans *Command* et choisir le protocole *UCI*. Toutes les cadences sont gérées : temps + incrément, nombre de coups (`movestogo`), temps fixe par coup, profondeur ou nombre de nœuds fixes.
- **Arena** : *Engines → Install New Engine*, sélectionner le binaire, choisir *UCI*.

Options UCI :

| Option | Défaut | Effet |
| --- | --- | --- |
| `Hash` | 16 | taille de la table de transposition, en Mio |
| `Move Overhead` | 30 | marge en ms gardée à chaque coup pour la latence de la GUI |
| `Clear Hash` | — | vide la table de transposition |

La recherche tourne dans un thread séparé : `stop` et `isready` répondent pendant la réflexion. Les logs du moteur partent sur stderr, ce qui n'interfère pas avec le protocole. `RUST_LOG=debug` les rend visibles.

Commandes de débogage :
- `go perft <n>` : nombre de nœuds par coup puis total (format Stockfish) ;
- `d` : affiche le plateau et la FEN ;
- `bench [profondeur]` (ou `chess-uci bench [profondeur]` en ligne de commande) : recherche à profondeur fixe sur 12 positions. Le nombre de nœuds sert de signature : il change dès que la recherche ou l'évaluation change.

Le moteur peut aussi être piloté par script : `printf "position startpos\ngo depth 10\n" | chess-uci` affiche le résultat complet avant de quitter.

## Feuille de route

- [x] **Phase 1** : squelette du workspace et du front, CI, Docker (avec `gpu-check`), justfile
- [x] **Phase 2** : `chess-core` complet (magic bitboards, coups légaux, make/unmake, Zobrist, FEN/SAN/UCI/PGN, fins de partie), perft exact
- [x] **Phase 3** : `chess-search` (PVS, iterative deepening, TT, quiescence, null move, LMR, killers, history, MVV-LVA, gestion du temps), évaluation PeSTO, UCI complet
- [x] **Phase 4** : `chess-server` (API REST sans état, analyse en direct par WebSocket, OpenAPI) + front : modes jeu et analyse
- [ ] **Phase 5** : `chess-train` : self-play, dataset, entraînement NNUE, export
- [ ] **Phase 6** : NNUE dans `chess-eval`, mode entraînement de l'UI, matchs entre versions
- [ ] **Phase 7** : optimisations (profiling, SIMD), README final

## Licence

MIT OR Apache-2.0
