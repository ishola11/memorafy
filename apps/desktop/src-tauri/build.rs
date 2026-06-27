fn main() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let pubkey_path = manifest_dir.join("keys").join("memora.key.pub");

    if pubkey_path.exists() {
        println!("cargo:rerun-if-changed={}", pubkey_path.display());
        if let Ok(pubkey) = std::fs::read_to_string(&pubkey_path) {
            let pubkey = pubkey.trim();
            if !pubkey.is_empty() {
                let config = serde_json::json!({
                    "plugins": {
                        "updater": {
                            "pubkey": pubkey
                        }
                    }
                });
                std::env::set_var("TAURI_CONFIG", config.to_string());
            }
        }
    }

    tauri_build::build()
}
