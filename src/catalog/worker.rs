//! Catalogue downloads off the UI thread, same shape as `online::worker`:
//! one thread per request, replies over a channel the event loop drains.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use super::{Index, Kind, Source};

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogEvent {
    Index(Result<Index, String>),
    /// A file fetched and validated, ready to write. `use_it` asks for it
    /// to become the current language/theme once installed.
    Fetched {
        kind: Option<Kind>,
        name: String,
        use_it: bool,
        result: Result<(Kind, String), String>,
    },
}

pub struct Fetcher {
    source: Source,
    tx: Sender<CatalogEvent>,
    rx: Receiver<CatalogEvent>,
    pending: usize,
}

impl Fetcher {
    pub fn new(base: &str) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            source: Source::new(base),
            tx,
            rx,
            pending: 0,
        }
    }

    pub fn busy(&self) -> bool {
        self.pending > 0
    }

    pub fn poll(&mut self) -> Vec<CatalogEvent> {
        let events: Vec<CatalogEvent> = self.rx.try_iter().collect();
        self.pending = self.pending.saturating_sub(events.len());
        events
    }

    pub fn fetch_index(&mut self) {
        let (src, tx) = (self.source.clone(), self.tx.clone());
        self.pending += 1;
        thread::spawn(move || {
            let _ = tx.send(CatalogEvent::Index(src.index()));
        });
    }

    /// Fetch `name`. Without a `kind` the index says which list it's in.
    pub fn fetch(&mut self, kind: Option<Kind>, name: String, use_it: bool) {
        let (src, tx) = (self.source.clone(), self.tx.clone());
        self.pending += 1;
        thread::spawn(move || {
            let result = match kind {
                Some(k) => Ok(k),
                None => src.index().and_then(|index| {
                    index
                        .kind_of(&name)
                        .ok_or_else(|| format!("`{name}` is not in the catalogue"))
                }),
            }
            .and_then(|k| src.fetch(k, &name).map(|text| (k, text)));
            let _ = tx.send(CatalogEvent::Fetched {
                kind,
                name,
                use_it,
                result,
            });
        });
    }
}
