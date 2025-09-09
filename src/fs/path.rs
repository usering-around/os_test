use core::{
    borrow::Borrow,
    ops::{Add, Deref},
    str,
};

use alloc::string::ToString;

use crate::alloc::string::String;
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct PathBuf {
    inner: String,
}

impl PathBuf {
    pub fn new(str: &str) -> PathBuf {
        PathBuf {
            inner: str.to_string(),
        }
    }
}

impl From<&Path> for PathBuf {
    fn from(value: &Path) -> Self {
        PathBuf::new(&value.inner)
    }
}

impl From<String> for PathBuf {
    fn from(value: String) -> Self {
        PathBuf { inner: value }
    }
}
impl Deref for PathBuf {
    type Target = Path;
    fn deref(&self) -> &Self::Target {
        Path::new(self.inner.as_str())
    }
}

impl AsRef<Path> for PathBuf {
    fn as_ref(&self) -> &Path {
        &self
    }
}

impl Borrow<Path> for PathBuf {
    fn borrow(&self) -> &Path {
        &self
    }
}

impl PathBuf {
    pub fn as_path(&self) -> &Path {
        &self
    }
}

impl Add<&Path> for PathBuf {
    type Output = PathBuf;
    fn add(self, rhs: &Path) -> Self::Output {
        PathBuf::from(self.inner + &rhs.inner)
    }
}

// transparent so that transmute is safe
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct Path {
    inner: str,
}

impl Path {
    pub fn new(str: &str) -> &Path {
        unsafe { core::mem::transmute(str) }
    }

    pub fn root() -> &'static Path {
        Path::new("/")
    }
    pub fn parent(&self) -> Option<&Path> {
        let Some((parent, _rest)) = self.inner.rsplit_once('/') else {
            return None;
        };
        if parent.is_empty() {
            // we're at the root - there's no parent left
            Some(Path::new("/"))
        } else {
            Some(Path::new(parent))
        }
    }

    /// Get the top folder, including the root directory.
    /// Returns None if the path has only 1 component and that component is not the root.
    /// i.e. for "/" it will return "/", and for "path" it will return None.
    pub fn top_folder(&self) -> Option<&Path> {
        if self.has_root() {
            return Some(Path::new("/"));
        }
        match self.inner.split_once('/') {
            Some((top, _rest)) => Some(Path::new(top)),
            None => None,
        }
    }

    pub fn filename(&self) -> Option<&Path> {
        if self.parent().is_some_and(|p| p != self) {
            let (_rest, filename) = self.inner.rsplit_once('/').unwrap();
            Some(Path::new(filename))
        } else {
            None
        }
    }

    pub fn split_from_top(&self) -> Option<(&Path, &Path)> {
        match self.inner.split_once('/') {
            Some((top, rest)) => Some((Path::new(top), Path::new(rest))),
            None => None,
        }
    }

    pub fn has_root(&self) -> bool {
        self.inner.starts_with('/')
    }

    pub fn is_root(&self) -> bool {
        &self.inner == "/"
    }

    pub fn relative_to(&self, relative: &Path) -> Option<&Path> {
        self.inner
            .strip_prefix(&relative.inner)
            .map(|e| e.strip_prefix("/").unwrap_or(e))
            .map(Path::new)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test_case]
    fn parents() {
        let path = Path::new("/usr/foo/tmp.html");
        let parent = path.parent().unwrap();
        let grand_parent = parent.parent().unwrap();
        assert_eq!(parent, Path::new("/usr/foo"));
        assert_eq!(grand_parent, Path::new("/usr"));
        assert_eq!(grand_parent.parent(), Some(Path::new("/")));
        assert_eq!(Path::new("hello").parent(), None);
    }

    #[test_case]
    fn top_folders() {
        let path = Path::new("/usr/foo/tmp.html");
        assert_eq!(path.top_folder(), Some(Path::new("/")));
        let path = Path::new("hello/byebyte/good/bye");
        assert_eq!(path.top_folder(), Some(Path::new("hello")));
        let path = Path::new("bye");
        assert_eq!(path.top_folder(), None);
        let path = Path::new("/");
        assert_eq!(path.top_folder(), Some(Path::new("/")));
    }

    #[test_case]
    fn relative() {
        let path = Path::new("/usr/foo/tmp/ok.html");
        let parent = Path::new("/usr/foo");
        assert_eq!(path.relative_to(parent), Some(Path::new("tmp/ok.html")));
        let path = Path::new("/text.html");
        let root = Path::root();
        assert_eq!(path.relative_to(root), Some(Path::new("text.html")));
        let path = Path::new("ok/test/testing");
        let unrelated_paarent = Path::new("/ok/test");
        assert_eq!(path.relative_to(unrelated_paarent), None);
    }
}
