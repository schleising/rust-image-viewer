use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Parent,
    Directory,
    Image,
    Video,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: EntryKind,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FolderView {
    pub path: PathBuf,
    pub entries: Vec<Entry>,
    pub highlighted: usize,
    pub scroll_top: f64,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct CatalogError {
    pub message: String,
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CatalogError {}

impl From<io::Error> for CatalogError {
    fn from(error: io::Error) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

pub fn normalize(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn list_folder(path: &Path) -> Result<FolderView, CatalogError> {
    let path = normalize(path);
    let mut directories = Vec::new();
    let mut files = Vec::new();
    let read = fs::read_dir(&path)?;
    for item in read {
        let item = match item {
            Ok(item) => item,
            Err(error) => {
                tracing::warn!(%error, "skipped a directory entry");
                continue;
            }
        };
        let name = item.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let child = item.path();
        let meta = match fs::metadata(&child) {
            Ok(meta) => meta,
            Err(error) => {
                tracing::warn!(%error, path = %child.display(), "skipped an unreadable entry");
                continue;
            }
        };
        if meta.is_dir() {
            if name.starts_with('.') {
                continue;
            }
            directories.push(child);
        } else if meta.is_file() && is_supported_file(&child) {
            if let Some(kind) = file_kind(name) {
                files.push((child, kind));
            }
        }
    }
    directories.sort_by(|a, b| cmp_path(a, b));
    files.sort_by(|a, b| cmp_path(&a.0, &b.0));

    let mut entries = Vec::new();
    if let Some(parent) = path.parent() {
        if parent != path {
            entries.push(Entry {
                label: parent_label(parent),
                path: parent.to_path_buf(),
                kind: EntryKind::Parent,
            });
        }
    }
    for directory in directories {
        let label = directory
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("folder")
            .to_string();
        entries.push(Entry {
            path: directory,
            kind: EntryKind::Directory,
            label,
        });
    }
    for (file, kind) in files {
        entries.push(Entry {
            label: file
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("file")
                .to_string(),
            path: file,
            kind,
        });
    }
    Ok(FolderView {
        path,
        entries,
        highlighted: 0,
        scroll_top: 0.0,
        error: None,
    })
}

pub fn file_kind(name: &str) -> Option<EntryKind> {
    let suffix = name.rsplit('.').next()?;
    if suffix == name {
        return None;
    }
    match suffix.to_ascii_lowercase().as_str() {
        "gif" | "jpg" | "jpeg" | "png" | "webp" => Some(EntryKind::Image),
        "mp4" | "mov" => Some(EntryKind::Video),
        _ => None,
    }
}

pub fn is_supported_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(file_kind)
        .is_some()
}

pub fn media_paths(folder: &FolderView) -> Vec<PathBuf> {
    folder
        .entries
        .iter()
        .filter(|entry| matches!(entry.kind, EntryKind::Image | EntryKind::Video))
        .map(|entry| entry.path.clone())
        .collect()
}

pub fn folder_stem(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("/")
        .to_string()
}

pub fn middle_elide(text: &str, max_chars: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if max_chars == 0 || chars.len() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let keep = max_chars - 1;
    let head = keep / 2;
    let tail = keep - head;
    let mut out: String = chars.iter().take(head).collect();
    out.push('…');
    out.extend(chars.iter().skip(chars.len() - tail));
    out
}

fn parent_label(parent: &Path) -> String {
    parent
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("/")
        .to_string()
}

fn cmp_path(a: &Path, b: &Path) -> Ordering {
    let a_name = a.file_name().and_then(|name| name.to_str()).unwrap_or("");
    let b_name = b.file_name().and_then(|name| name.to_str()).unwrap_or("");
    cmp_name(a_name, b_name)
}

fn cmp_name(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

pub fn pictures_or_home(home: &Path, pictures_exists: bool) -> PathBuf {
    if pictures_exists {
        home.join("Pictures")
    } else {
        home.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = TEMP_SEQ.fetch_add(1, Ordering::Relaxed);
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let path = std::env::temp_dir().join(format!("image-viewer-catalog-{millis}-{n}"));
            fs::create_dir_all(&path).unwrap_or(());
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn touch(path: &Path) {
        fs::write(path, b"x").unwrap_or(());
    }

    #[test]
    fn sort_hidden_parent_suffix_and_dotfiles() -> Result<(), CatalogError> {
        let dir = TempDir::new();
        fs::create_dir_all(dir.0.join("zeta"))?;
        fs::create_dir_all(dir.0.join("Alpha"))?;
        fs::create_dir_all(dir.0.join(".secret"))?;
        touch(&dir.0.join("b.PNG"));
        touch(&dir.0.join("A.jpg"));
        touch(&dir.0.join(".hidden.gif"));
        touch(&dir.0.join("notes.txt"));
        touch(&dir.0.join(".notes.txt"));
        touch(&dir.0.join("clip.MOV"));

        let folder = list_folder(&dir.0)?;
        let labels: Vec<_> = folder
            .entries
            .iter()
            .map(|entry| (entry.kind, entry.label.as_str()))
            .collect();
        let parent = dir.0.parent().ok_or_else(|| CatalogError {
            message: "temp dir has no parent".into(),
        })?;
        assert_eq!(labels[0].0, EntryKind::Parent);
        assert_eq!(labels[0].1, parent_label(parent));
        assert_ne!(labels[0].1, "..");
        assert_eq!(
            labels[1..],
            [
                (EntryKind::Directory, "Alpha"),
                (EntryKind::Directory, "zeta"),
                (EntryKind::Image, ".hidden"),
                (EntryKind::Image, "A"),
                (EntryKind::Image, "b"),
                (EntryKind::Video, "clip"),
            ]
        );
        Ok(())
    }

    #[test]
    fn root_omits_parent() -> Result<(), CatalogError> {
        let folder = list_folder(Path::new("/"))?;
        assert!(folder
            .entries
            .iter()
            .all(|entry| entry.kind != EntryKind::Parent));
        Ok(())
    }

    #[test]
    fn middle_elide_keeps_both_ends() {
        assert_eq!(middle_elide("Holiday", 20), "Holiday");
        assert_eq!(middle_elide("abcdefghij", 7), "abc…hij");
    }
}
