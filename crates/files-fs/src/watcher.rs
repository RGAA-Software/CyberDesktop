use std::path::Path;
use std::time::Duration;

use flume::Receiver;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// Keeps a `notify` watcher alive for a single directory.
pub struct DirectoryWatcher {
    _watcher: RecommendedWatcher,
    _debounce_thread: std::thread::JoinHandle<()>,
}

impl DirectoryWatcher {
    /// Returns a debounced receiver: each message means the directory may have changed.
    pub fn watch(path: &Path, debounce: Duration) -> anyhow::Result<(Self, Receiver<()>)> {
        Self::watch_mode(path, RecursiveMode::NonRecursive, debounce)
    }

    /// Watches a directory tree (e.g. Quick Access `AutomaticDestinations`).
    pub fn watch_recursive(
        path: &Path,
        debounce: Duration,
    ) -> anyhow::Result<(Self, Receiver<()>)> {
        Self::watch_mode(path, RecursiveMode::Recursive, debounce)
    }

    fn watch_mode(
        path: &Path,
        mode: RecursiveMode,
        debounce: Duration,
    ) -> anyhow::Result<(Self, Receiver<()>)> {
        let (raw_tx, raw_rx) = flume::bounded::<()>(64);
        let mut watcher = RecommendedWatcher::new(
            move |result: notify::Result<notify::Event>| {
                if result.is_ok() {
                    let _ = raw_tx.send(());
                }
            },
            notify::Config::default(),
        )?;
        watcher.watch(path, mode)?;

        let (debounced_tx, debounced_rx) = flume::bounded(1);
        let debounce_thread = std::thread::spawn(move || {
            while raw_rx.recv().is_ok() {
                std::thread::sleep(debounce);
                while raw_rx.try_recv().is_ok() {}
                let _ = debounced_tx.send(());
            }
        });

        Ok((
            Self {
                _watcher: watcher,
                _debounce_thread: debounce_thread,
            },
            debounced_rx,
        ))
    }
}
