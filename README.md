# AIOS Official Store

Community-driven decentralized app index for AIOS WASM blocks.

## Submitting a Block

1. Fork this repository
2. Create a directory: locks/your-block-name/
3. Add:
   - lock.wasm — compiled WASM binary
   - manifest.json — block metadata
   - README.md — description (EN + RU)
4. Open a Pull Request

## Manifest Template

`json
{
  "name": "my-block",
  "version": "1.0.0",
  "description": {
    "en": "Description in English",
    "ru": "Описание на русском"
  },
  "author": "your-github-handle",
  "capabilities": ["filesystem:read"],
  "wasm_hash": "sha256:..."
}
`

## License

MIT (index and templates) — Blocks retain their own licenses.
