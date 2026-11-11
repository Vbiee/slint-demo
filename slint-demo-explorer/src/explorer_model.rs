use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Clone)]
pub struct Entry {
    pub path: PathBuf,
    pub name: String,
    pub type_name: String,
    pub modified: String,
    pub size: String,
    pub is_folder: bool,
}

/// Filesystem-only model: no Slint, Winit, or rendering dependency.
pub struct ExplorerModel {
    location: PathBuf,
}

impl ExplorerModel {
    pub fn new() -> Self {
        Self {
            location: Self::default_location(),
        }
    }

    pub fn default_location() -> PathBuf {
        std::env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .filter(|path| path.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn location(&self) -> &Path {
        &self.location
    }

    pub fn navigate(&mut self, destination: impl AsRef<Path>) -> bool {
        let path = destination.as_ref();
        if path.is_dir() {
            self.location = path.to_path_buf();
            true
        } else {
            false
        }
    }

    pub fn navigate_up(&mut self) -> bool {
        let parent = self.location.parent().map(Path::to_path_buf);
        parent.map(|parent| self.navigate(parent)).unwrap_or(false)
    }

    pub fn entries(&self) -> Vec<Entry> {
        let mut entries = fs::read_dir(&self.location)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|item| Self::entry(item.path()).ok())
            .collect::<Vec<_>>();
        entries.sort_by(|a, b| {
            b.is_folder
                .cmp(&a.is_folder)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        entries
    }

    fn entry(path: PathBuf) -> std::io::Result<Entry> {
        let metadata = fs::metadata(&path)?;
        let is_folder = metadata.is_dir();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let type_name = if is_folder {
            "File folder".into()
        } else {
            path.extension()
                .map(|ext| format!("{} file", ext.to_string_lossy().to_uppercase()))
                .unwrap_or_else(|| "File".into())
        };
        Ok(Entry {
            path,
            name,
            type_name,
            modified: modified_text(metadata.modified().ok()),
            size: if is_folder {
                String::new()
            } else {
                format_size(metadata.len())
            },
            is_folder,
        })
    }
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn modified_text(time: Option<SystemTime>) -> String {
    time.and_then(|time| time.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|duration| format!("{} sec ago", duration.as_secs()))
        .unwrap_or_else(|| "Unknown".into())
}
