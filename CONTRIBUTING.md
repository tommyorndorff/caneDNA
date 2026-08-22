# Contributing to caneDNA

Thanks for your interest! caneDNA is an open taper explorer for split-bamboo fly
rods. Start with [`AGENTS.md`](AGENTS.md) for project context and
[`HOWTOAI.md`](HOWTOAI.md) for the practical build/run/regenerate commands.

## Development setup

- **Rust** (stable) for the app: `cargo test -p roddna-core`, `cargo run -p roddna-gui`.
- **Python 3 + `openpyxl`** to regenerate the taper library from `data/raw/`.
- **Web build (optional):** `rustup target add wasm32-unknown-unknown` and
  `cargo install --locked trunk`, then `cd crates/roddna-gui && trunk serve`.

## Ground rules

- **Conventional Commits.** Use `feat:`, `fix:`, `chore:`, `docs:`, etc. Releases
  and the changelog are generated from commit messages (release-please).
- **Don't hand-edit generated data.** `data/tapers.json` and `data/sources/*.json`
  are produced by the scripts in `scripts/`. Change an importer or
  `build_library.py` and re-run the pipeline instead.
- **Preserve attribution.** Every taper carries a `provenance` block. Keep it
  intact, and if you add a new data source, credit it in the Data Sources table
  in `README.md` and note its license.
- **Keep it building on both targets.** For UI changes, confirm `cargo build
  -p roddna-gui` and, when practical, `trunk build` both succeed.

## Pull requests

1. Branch off `main`.
2. Make your change; run `cargo test -p roddna-core` (and rebuild the GUI if you
   touched data or UI).
3. Open a PR with a clear, conventional-commit-style title.

## Data licensing

Taper data comes from multiple sources with differing licenses (see `README.md`).
Please respect each source's terms and preserve attribution.
