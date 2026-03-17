use std::path::PathBuf;

pub type FileId = u32;

#[derive(Debug, Clone, Default)]
pub struct FileTable {
    files: Vec<(PathBuf, String)>,
}

impl FileTable {
    pub fn new() -> Self {
        Self { files: Vec::new() }
    }

    pub fn add(&mut self, path: PathBuf, source: String) -> FileId {
        let id = self.files.len() as FileId;
        self.files.push((path, source));
        id
    }

    pub fn source(&self, id: FileId) -> &str {
        &self.files[id as usize].1
    }

    pub fn path(&self, id: FileId) -> &PathBuf {
        &self.files[id as usize].0
    }

    pub fn iter(&self) -> impl Iterator<Item = (FileId, &PathBuf, &str)> {
        self.files
            .iter()
            .enumerate()
            .map(|(index, (path, source))| (index as FileId, path, source.as_str()))
    }
}