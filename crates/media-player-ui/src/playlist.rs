use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct Playlist {
    items: Vec<PathBuf>,
    current_index: Option<usize>,
}

impl Playlist {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            current_index: None,
        }
    }

    pub fn from_paths(paths: Vec<PathBuf>) -> Self {
        let current_index = if paths.is_empty() { None } else { Some(0) };
        Self {
            items: paths,
            current_index,
        }
    }

    pub fn add(&mut self, path: PathBuf) {
        if self.items.is_empty() {
            self.current_index = Some(0);
        }
        self.items.push(path);
    }

    pub fn items(&self) -> &[PathBuf] {
        &self.items
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn current(&self) -> Option<&PathBuf> {
        self.current_index.and_then(|i| self.items.get(i))
    }

    pub fn select(&mut self, index: usize) -> Option<&PathBuf> {
        if index < self.items.len() {
            self.current_index = Some(index);
            self.current()
        } else {
            None
        }
    }

    pub fn next(&mut self) -> Option<&PathBuf> {
        let idx = self.current_index? + 1;
        if idx < self.items.len() {
            self.current_index = Some(idx);
            self.current()
        } else {
            None
        }
    }

    pub fn prev(&mut self) -> Option<&PathBuf> {
        let idx = self.current_index?.checked_sub(1)?;
        self.current_index = Some(idx);
        self.current()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}
