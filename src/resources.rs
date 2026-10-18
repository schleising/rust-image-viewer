use std::path::PathBuf;

pub fn resources_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(resources) = exe
            .parent()
            .and_then(|path| path.parent())
            .map(|path| path.join("Resources"))
        {
            if resources.is_dir() {
                return resources;
            }
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets")
}

pub fn model_path(scale: u8) -> PathBuf {
    resources_dir()
        .join("models")
        .join(format!("fsrcnn_x{scale}.onnx"))
}

pub fn icon_path(name: &str) -> Option<PathBuf> {
    match name {
        "folder.png" | "loading.png" | "video.png" => Some(resources_dir().join(name)),
        _ => None,
    }
}
