//! Peer-page context: the same catalogue renders local and mounted pages,
//! with the pause in force that every page shows.
//! URLs are constructed at their source, including query strings, rather
//! than rewriting rendered HTML (which could also rewrite user filenames).

use crate::Mount;

pub(crate) struct Renderer {
    inner: pigeon_i18n::render::Renderer,
    pub mount: Option<Mount>,
    /// The pause in force, which the page shows as a banner (15 §15.6).
    pub banner: Option<Banner>,
}

/// A pause banner: what the pause is, and the session's form token for
/// its *Share again*.
#[derive(Clone, Debug)]
pub(crate) struct Banner {
    /// The pause, such as "Sharing is paused until 14:38."
    pub message: pigeon_proto::Message,
    /// The session's form token.
    pub form: String,
}

impl Renderer {
    pub fn new(catalogue: pigeon_i18n::Catalogue, now: i64, mount: Option<Mount>) -> Self {
        Self {
            inner: pigeon_i18n::render::Renderer::new(catalogue, now),
            mount,
            banner: None,
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
