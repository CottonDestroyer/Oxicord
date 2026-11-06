//! Serves Discord's own icons from a `discord-assets` folder when one exists,
//! and everything else from the bundled catalog. The icons are Discord's, so
//! they are not part of the repository: with no folder the app keeps drawing
//! the bundled ones.

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use gpui_kit::{AssetSource, SharedString};

/// Where `discord-assets` is looked for: beside the executable, then in the
/// working directory.
pub fn folder() -> Option<PathBuf> {
    let beside_exe = std::env::current_exe().ok().and_then(|exe| Some(exe.parent()?.join("discord-assets")));
    beside_exe
        .into_iter()
        .chain([PathBuf::from("discord-assets")])
        .find(|dir| dir.is_dir())
}

pub struct Assets<A> {
    inner: A,
    icons: Option<PathBuf>,
}

impl<A> Assets<A> {
    pub fn new(inner: A) -> Self {
        Self { inner, icons: folder().map(|dir| dir.join("icons")) }
    }

    fn override_for(&self, path: &str) -> Option<Vec<u8>> {
        let name = Path::new(path.strip_prefix("icons/")?).file_name()?;
        std::fs::read(self.icons.as_ref()?.join(name)).ok()
    }
}

impl<A: AssetSource> AssetSource for Assets<A> {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        match self.override_for(path) {
            Some(bytes) => Ok(Some(Cow::Owned(bytes))),
            None => self.inner.load(path),
        }
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        self.inner.list(path)
    }
}
