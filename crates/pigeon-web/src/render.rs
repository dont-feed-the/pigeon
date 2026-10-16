//! Peer-page context: the same catalogue renders local and mounted pages.
//! URLs are constructed at their source, including query strings, rather
//! than rewriting rendered HTML (which could also rewrite user filenames).

use crate::Mount;

pub(crate) struct Renderer {
    inner: pigeon_i18n::render::Renderer,
    pub mount: Option<Mount>,
}

impl Renderer {
    pub fn new(catalogue: pigeon_i18n::Catalogue, now: i64, mount: Option<Mount>) -> Self {
        Self {
            inner: pigeon_i18n::render::Renderer::new(catalogue, now),
            mount,
        }
    }

    pub fn url(&self, path: &str) -> String {
        self.mount
            .as_ref()
            .map_or_else(|| path.to_owned(), |mount| format!("{}{path}", mount.path))
    }

    pub fn storage_controls(&self) -> bool {
        self.mount.as_ref().is_none_or(|m| m.storage_controls)
    }
}

impl std::ops::Deref for Renderer {
    type Target = pigeon_i18n::render::Renderer;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
