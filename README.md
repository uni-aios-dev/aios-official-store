# AIOS Official Store

Community-driven WASM app registry for AIOS. This repository contains both the
published block binaries and the Rust sources used to build them.

## Layout

```
index.json                       generated catalog consumed by aios-store
blocks/<name>/block.wasm         compiled guest module
blocks/<name>/manifest.json      source metadata (EN + RU description)
blocks/<name>/README.md          block documentation
sdk/aios-block-sdk/              shared guest ABI helper
tools/indexgen/                  index.json generator
```

Two block implementations are included as separate crates:

- `blocks/calculator` — expression calculator with `ans`, variables and history.
- `blocks/minesweeper` — console minesweeper with flood fill and flags.

Both are stateful across `block run` calls because the AIOS host keeps one
`WasmBlock` instance per installed block.

## Build and publish the catalog

Run these commands from the repository root on Windows:

```powershell
$env:Path = [System.Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [System.Environment]::GetEnvironmentVariable("Path","User")
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown -p aios-block-calculator -p aios-block-minesweeper
Copy-Item target\wasm32-unknown-unknown\release\aios_block_calculator.wasm blocks\calculator\block.wasm
Copy-Item target\wasm32-unknown-unknown\release\aios_block_minesweeper.wasm blocks\minesweeper\block.wasm
cargo run -p aios-store-indexgen
```

The generator rewrites `index.json` and fills `wasm_hash` (`sha256:` prefix) and
`wasm_size_bytes` from the actual binaries. Run the tests with
`cargo test --workspace`.

## Manifest format

`manifest.json` next to each binary uses this shape:

```json
{
  "name": "my-block",
  "version": "1.0.0",
  "description": {
    "en": "Description in English",
    "ru": "Описание на русском"
  },
  "author": "your-github-handle",
  "capabilities": [],
  "wasm_hash": "sha256:...",
  "wasm_size_bytes": 0
}
```

`description` may also be a plain string. Capabilities use AIOS tokens such
as `CAP_FS_READ`; the host also accepts `filesystem:read`-style aliases.
`index.json` is either the object produced by `indexgen` (with a `blocks`
array) or a plain array of these manifests. The catalog loader also supports
the legacy flat layout `<name>_<version>.wasm`, but new blocks should use the
nested `blocks/<name>/block.wasm` layout above.

## Running a block in AIOS

```
store source local:<path-to-aios-official-store>
store install calculator
block run calculator 2+3*4
block run calculator ans+1
block run minesweeper new 9 9 10
block run minesweeper open 1 1
```

`block run` is required for interactive blocks: the older `wasm call` command
sends an empty payload and creates a fresh instance for every call.

## Submitting a block

1. Fork this repository.
2. Create `blocks/your-block-name/` with `block.wasm`, `manifest.json` and a
   bilingual `README.md`.
3. Regenerate `index.json` with `tools/indexgen`.
4. Open a pull request.

## License

MIT (index and templates) — blocks retain their own licenses.
