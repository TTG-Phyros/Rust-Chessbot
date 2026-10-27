# Opening names

`a.tsv` to `e.tsv` come unchanged from [lichess-org/chess-openings](https://github.com/lichess-org/chess-openings)
at commit `f06a50d5c7c140e50b068049fa31d289e2c3d347` (2026-10-27), released under CC0 1.0
(`COPYING.txt`). Each line is an ECO code, a name and the moves of the line (`1. d4 d5 2. Bf4`).

They are compiled into `chess-openings` (`include_str!`). To update them, download the five
files of a newer commit over these ones, update the commit above and run
`cargo test -p chess-openings`.
