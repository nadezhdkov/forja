use std::path::Path;

use forja_core::{ForjaError, DEFAULT_TEMPLATE};

/// `forja init` — write a commented `forja.toml` scaffold (RF-03).
pub fn run(path: &Path, force: bool) -> Result<(), ForjaError> {
    if path.exists() && !force {
        return Err(ForjaError::ConfigAlreadyExists {
            path: path.to_path_buf(),
        });
    }

    std::fs::write(path, DEFAULT_TEMPLATE).map_err(|source| ForjaError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    println!("wrote {}", path.display());
    Ok(())
}
