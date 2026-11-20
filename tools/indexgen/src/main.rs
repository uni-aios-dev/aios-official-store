use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const STORE_DESCRIPTION: &str = "Community-driven WASM app registry for AIOS";
const STORE_DESCRIPTION_RU: &str = "Сообщественный реестр WASM-блоков для AIOS";

fn store_root() -> PathBuf {
    if let Some(root) = env::args_os().nth(1) {
        return PathBuf::from(root);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("tool must live under the store root")
        .to_path_buf()
}

fn collect_blocks(root: &Path) -> Result<Vec<serde_json::Value>, String> {
    let blocks_dir = root.join("blocks");
    let mut entries = fs::read_dir(&blocks_dir)
        .map_err(|error| format!("cannot read {}: {error}", blocks_dir.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    entries.sort();

    let mut blocks = Vec::new();
    for dir in entries {
        let wasm_path = dir.join("block.wasm");
        let manifest_path = dir.join("manifest.json");
        if !wasm_path.is_file() || !manifest_path.is_file() {
            continue;
        }
        let binary = fs::read(&wasm_path)
            .map_err(|error| format!("cannot read {}: {error}", wasm_path.display()))?;
        let text = fs::read_to_string(&manifest_path)
            .map_err(|error| format!("cannot read {}: {error}", manifest_path.display()))?;
        let mut manifest: serde_json::Value = serde_json::from_str(&text)
            .map_err(|error| format!("invalid {}: {error}", manifest_path.display()))?;
        let hash = hex::encode(Sha256::digest(&binary));
        if let Some(object) = manifest.as_object_mut() {
            object.insert(
                "wasm_hash".into(),
                serde_json::json!(format!("sha256:{hash}")),
            );
            object.insert("wasm_size_bytes".into(), serde_json::json!(binary.len()));
            let order = [
                "name",
                "version",
                "description",
                "author",
                "capabilities",
                "wasm_hash",
                "wasm_size_bytes",
            ];
            let mut ordered = serde_json::Map::new();
            for key in order {
                if let Some(value) = object.remove(key) {
                    ordered.insert(key.to_string(), value);
                }
            }
            for (key, value) in object.iter() {
                ordered.insert(key.clone(), value.clone());
            }
            *object = ordered;
        }
        blocks.push(manifest);
    }
    Ok(blocks)
}

fn main() {
    let root = store_root();
    let blocks = match collect_blocks(&root) {
        Ok(blocks) => blocks,
        Err(error) => {
            eprintln!("indexgen: {error}");
            std::process::exit(1);
        }
    };
    let index = serde_json::json!({
        "schema_version": "1.0",
        "name": "aios-official-store",
        "description": STORE_DESCRIPTION,
        "description_ru": STORE_DESCRIPTION_RU,
        "blocks": blocks,
    });
    let path = root.join("index.json");
    let text = format!(
        "{}\n",
        serde_json::to_string_pretty(&index).expect("serialize index")
    );
    if let Err(error) = fs::write(&path, text) {
        eprintln!("indexgen: cannot write {}: {error}", path.display());
        std::process::exit(1);
    }
    println!(
        "indexgen: wrote {} block(s) to {}",
        index["blocks"].as_array().map_or(0, Vec::len),
        path.display()
    );
}
