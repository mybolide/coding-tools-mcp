use std::env;
use std::path::{Path, PathBuf};

pub fn resolve_from_path(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let paths = env::split_paths(&path_var);
    let windows = cfg!(windows);
    let candidates = if windows {
        vec![name.to_string(), format!("{name}.exe")]
    } else {
        vec![name.to_string()]
    };

    for dir in paths {
        for candidate in &candidates {
            let full = dir.join(candidate);
            if full.is_file() {
                return Some(full);
            }
        }
    }
    None
}

pub fn append_if_exists(paths: &mut Vec<PathBuf>, candidate: impl AsRef<Path>) {
    let candidate = candidate.as_ref();
    if candidate.is_file() {
        paths.push(candidate.to_path_buf());
    }
}
