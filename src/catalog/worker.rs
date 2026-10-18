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
    /// A language module's file, fetched and validated.
    ModuleFetched {
        language: String,
        module: String,
        result: Result<String, String>,
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
        let src = self.source.clone();
        self.pending += 1;
        spawn(
            self.tx.clone(),
            |e| CatalogEvent::Index(Err(e)),
            move || CatalogEvent::Index(src.index()),
        );
    }

    /// Fetch one of `language`'s modules.
    pub fn fetch_module(&mut self, language: String, module: String) {
        let src = self.source.clone();
        self.pending += 1;
        let (l, m) = (language.clone(), module.clone());
        spawn(
            self.tx.clone(),
            |e| CatalogEvent::ModuleFetched {
                language: l,
                module: m,
                result: Err(e),
            },
            move || {
                let result = src.fetch_module(&language, &module);
                CatalogEvent::ModuleFetched {
                    language,
                    module,
                    result,
                }
            },
        );
    }

    /// Fetch `name`. Without a `kind` the index says which list it's in.
    pub fn fetch(&mut self, kind: Option<Kind>, name: String, use_it: bool) {
        let src = self.source.clone();
        self.pending += 1;
        let n = name.clone();
        spawn(
            self.tx.clone(),
            move |e| CatalogEvent::Fetched {
                kind,
                name: n,
                use_it,
                result: Err(e),
            },
            move || {
                let result = match kind {
                    Some(k) => Ok(k),
                    None => src.index().and_then(|index| {
                        index
                            .kind_of(&name)
                            .ok_or_else(|| format!("`{name}` is not in the catalogue"))
                    }),
                }
                .and_then(|k| src.fetch(k, &name).map(|text| (k, text)));
                CatalogEvent::Fetched {
                    kind,
                    name,
                    use_it,
                    result,
                }
            },
        );
    }
}

/// Run `work` on its own thread and send what it returns. If it panics,
/// `failed` makes the reply instead, so `pending` still drops back.
fn spawn(
    tx: Sender<CatalogEvent>,
    failed: impl FnOnce(String) -> CatalogEvent + Send + 'static,
    work: impl FnOnce() -> CatalogEvent + Send + 'static,
) {
    thread::spawn(move || {
        let ev = crate::errlog::catch_panic("catalog", work)
            .unwrap_or_else(|msg| failed(format!("internal error: {msg}")));
        let _ = tx.send(ev);
    });
}
