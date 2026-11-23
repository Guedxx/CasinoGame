#[cfg(target_arch = "wasm32")]
fn storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or("No browser window")?
        .local_storage()
        .map_err(|e| format!("{e:?}"))?
        .ok_or("Local storage is disabled".into())
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn read(key: &str) -> Result<Option<String>, String> {
    storage()?
        .get_item(key)
        .map_err(|e| format!("{e:?}"))
}
#[cfg(target_arch = "wasm32")]
pub(crate) fn write(key: &str, value: &str) -> Result<(), String> {
    storage()?
        .set_item(key, value)
        .map_err(|e| format!("{e:?}"))
}
#[cfg(not(target_arch = "wasm32"))]
fn settings_path(key: &str) -> Result<std::path::PathBuf, String> {
    use std::{env, path::PathBuf};
    let root = if cfg!(target_os = "windows") {
        env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"))
    } else {
        env::var_os("XDG_CONFIG_HOME")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
    }
    .ok_or("No user configuration directory")?;
    Ok(root.join("CasinoGame").join(format!("{key}.json")))
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read(key: &str) -> Result<Option<String>, String> {
    match std::fs::read_to_string(settings_path(key)?) {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write(key: &str, value: &str) -> Result<(), String> {
    let path = settings_path(key)?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&temporary, value).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, path).map_err(|e| e.to_string())
}
