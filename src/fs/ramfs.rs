use super::path::{Path, PathBuf};
use super::vfs::{File, FileSystem, Result, VfsError};
use crate::alloc::sync::{Arc, Weak};
use crate::alloc::{boxed::Box, vec::Vec};
use crate::fs::vfs::{DirEntry, FileType};
use spin::rwlock::RwLock;

#[derive(Clone, Debug)]
pub struct RamfsFileHandle {
    inner: Arc<RamfsFile>,
    pos: usize,
}

impl PartialEq for RamfsFileHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl RamfsFileHandle {
    fn new(file: Arc<RamfsFile>) -> Self {
        RamfsFileHandle {
            inner: file,
            pos: 0,
        }
    }
}

#[derive(Debug)]
struct RamfsFile {
    name: PathBuf,
    data: RwLock<Vec<u8>>,
    // perhaps we'll use this in the future for RamfsFile::Delete
    #[allow(unused)]
    parent: Weak<Dir>,
}

struct Dir {
    name: PathBuf,
    entries: RwLock<Vec<RamfsDirEntry>>,
    // perhaps we'll use this in the future for Dir::Delete
    #[allow(unused)]
    parent: Weak<Dir>,
}

impl Dir {
    // find a directory relative to a path
    fn find_dir(&self, path: &Path) -> Option<Arc<Dir>> {
        let entries: spin::RwLockReadGuard<'_, Vec<RamfsDirEntry>> = self.entries.read();
        for entry in entries.iter() {
            if let Some((top, rest)) = path.split_from_top() {
                // we have a top and a a rest, recursively search
                if let RamfsDirEntry::Dir(dir) = entry {
                    if dir.name.as_path() == top {
                        return dir.find_dir(rest);
                    }
                }
            } else {
                // we're left with just the file name, dir name
                if let RamfsDirEntry::Dir(dir) = entry {
                    if dir.name.as_path() == path {
                        return Some(dir.clone());
                    }
                }
            }
        }
        None
    }
    fn find_file(&self, path: &Path) -> Option<RamfsFileHandle> {
        let entries: spin::RwLockReadGuard<'_, Vec<RamfsDirEntry>> = self.entries.read();
        for entry in entries.iter() {
            if let Some((top, rest)) = path.split_from_top() {
                // we have a top and a a rest, recursively search
                if let RamfsDirEntry::Dir(dir) = entry {
                    if dir.name.as_path() == top {
                        return dir.find_file(rest);
                    }
                }
            } else {
                // we're left with just the file name, we'll check if it can be found in the current directory
                if let RamfsDirEntry::File(file) = entry {
                    if file.name.as_path() == path {
                        return Some(RamfsFileHandle::new(file.clone()));
                    }
                }
            }
        }
        None
    }
}

enum RamfsDirEntry {
    Dir(Arc<Dir>),
    File(Arc<RamfsFile>),
}

impl RamfsDirEntry {
    fn name(&self) -> &Path {
        match self {
            RamfsDirEntry::Dir(dir) => dir.name.as_path(),
            RamfsDirEntry::File(file) => file.name.as_path(),
        }
    }

    fn file_type(&self) -> FileType {
        match self {
            RamfsDirEntry::Dir(_) => FileType::Directory,
            RamfsDirEntry::File(_) => FileType::File,
        }
    }
}
pub struct Ramfs {
    root: Arc<Dir>,
}

impl Ramfs {
    pub fn new() -> Self {
        let root: Arc<Dir> = Arc::new_cyclic(|this| Dir {
            name: PathBuf::new("/"),
            entries: RwLock::new(Vec::new()),
            parent: this.clone(),
        });
        Ramfs { root }
    }
}

impl File for RamfsFileHandle {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let mut read = 0;
        for (ptr, byte) in buf
            .iter_mut()
            .zip(self.inner.data.read().iter().skip(self.pos))
        {
            *ptr = *byte;
            read += 1;
        }
        self.pos += read;
        Ok(read)
    }
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut wrote = 0;
        let mut vec: spin::RwLockWriteGuard<Vec<u8>> = self.inner.data.write();
        for byte in buf {
            vec.insert(self.pos + wrote, *byte);
            wrote += 1;
        }
        self.pos += wrote;
        Ok(wrote)
    }
}

impl FileSystem for Ramfs {
    type File = RamfsFileHandle;

    fn file_type(&self, path: &Path) -> Result<FileType> {
        if !path.has_root() {
            return Err(VfsError::PathIsNotAbsolute);
        }
        if path.is_root() {
            Ok(FileType::Directory)
        } else {
            // if the path is not root, it has a parent -
            let parent = path.parent().unwrap();
            let parent_dir = if parent.is_root() {
                self.root.clone()
            } else {
                let Some(parent_dir) = self
                    .root
                    .find_dir(parent.relative_to(Path::root()).unwrap())
                else {
                    return Err(VfsError::PathDoesNotExist);
                };
                parent_dir
            };

            let name = path.filename().unwrap();
            if let Some(entry) = parent_dir.entries.read().iter().find(|e| e.name() == name) {
                Ok(entry.file_type())
            } else {
                Err(VfsError::PathDoesNotExist)
            }
        }
    }
    fn open_file(&self, path: &Path) -> Result<Self::File> {
        if !path.has_root() {
            return Err(VfsError::PathIsNotAbsolute);
        }
        if let Some((_root, rest)) = path.split_from_top() {
            match self.root.find_file(rest) {
                Some(file) => Ok(file),
                None => Err(VfsError::PathDoesNotExist),
            }
        } else {
            Err(VfsError::PathDoesNotHaveAFilename)
        }
    }
    // create a file from an absolute path (path with root)
    fn create_file(&self, path: &Path) -> Result<Self::File> {
        if !path.has_root() {
            return Err(VfsError::PathIsNotAbsolute);
        }
        let Some(parent) = path.parent() else {
            return Err(VfsError::PathDoesNotHaveAFilename);
        };
        let dir = if parent.is_root() {
            self.root.clone()
        } else {
            let Some(dir) = self.root.find_dir(parent.split_from_top().unwrap().1) else {
                return Err(VfsError::DirectoryDoesNotExist);
            };
            dir
        };

        let file = Arc::new(RamfsFile {
            name: PathBuf::from(path.filename().unwrap()),
            data: RwLock::new(Vec::new()),
            parent: Arc::downgrade(&dir),
        });
        dir.entries.write().push(RamfsDirEntry::File(file.clone()));
        Ok(RamfsFileHandle::new(file))
    }

    fn create_dir(&self, path: &Path) -> Result<()> {
        if !path.has_root() {
            return Err(VfsError::PathIsNotAbsolute);
        }
        let Some(parent) = path.parent() else {
            // if there isn't a parent then this path must be the root path
            return Err(VfsError::PathAlreadyExists);
        };
        let dir = if parent.is_root() {
            self.root.clone()
        } else {
            let Some(dir) = self.root.find_dir(parent.split_from_top().unwrap().1) else {
                return Err(VfsError::DirectoryDoesNotExist);
            };
            dir
        };
        let new_dir = Arc::new(Dir {
            name: PathBuf::from(path.filename().unwrap()),
            entries: RwLock::new(Vec::new()),
            parent: Arc::downgrade(&dir),
        });
        dir.entries.write().push(RamfsDirEntry::Dir(new_dir));
        Ok(())
    }

    fn delete(&self, path: &Path) -> Result<()> {
        if !path.has_root() {
            return Err(VfsError::PathIsNotAbsolute);
        }
        if path.is_root() {
            self.root.entries.write().clear();
            Ok(())
        } else {
            // if the path is not root, it has a parent -
            let parent = path.parent().unwrap();
            let parent_dir = if parent.is_root() {
                self.root.clone()
            } else {
                let Some(parent_dir) = self
                    .root
                    .find_dir(parent.relative_to(Path::root()).unwrap())
                else {
                    return Err(VfsError::PathDoesNotExist);
                };
                parent_dir
            };

            let name = path.filename().unwrap();
            parent_dir.entries.write().retain(|e| e.name() != name);
            Ok(())
        }
    }

    fn open_dir(&self, path: &Path) -> Result<Box<dyn Iterator<Item = DirEntry>>> {
        if !path.has_root() {
            return Err(VfsError::PathIsNotAbsolute);
        }
        let dir = if path.is_root() {
            self.root.clone()
        } else {
            let Some(dir) = self.root.find_dir(path.relative_to(Path::root()).unwrap()) else {
                return Err(VfsError::PathDoesNotExist);
            };
            dir
        };
        let entries = dir
            .entries
            .read()
            .iter()
            .map(|e| DirEntry {
                path: PathBuf::from(path) + e.name(),
                file_type: e.file_type(),
            })
            .collect::<Vec<DirEntry>>();
        Ok(Box::new(entries.into_iter()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test_case]
    fn basic_ramfs() {
        let ramfs = Ramfs::new();
        let path = Path::new("/hello.txt");
        assert_eq!(ramfs.open_file(path), Err(VfsError::PathDoesNotExist));
        let wrote = b"hello, world!";
        {
            let mut file = ramfs.create_file(path).unwrap();
            assert_eq!(file.write(wrote).unwrap(), wrote.len());
        }
        let mut file = ramfs.open_file(path).unwrap();
        let mut buf = [0; 13];
        assert_eq!(file.read(&mut buf).unwrap(), wrote.len());
        assert_eq!(buf, *wrote);

        let dir_path = Path::new("/hello");
        ramfs.create_dir(dir_path).unwrap();
        ramfs.create_file(Path::new("/hello/test.txt")).unwrap();

        assert_eq!(
            ramfs.create_file(Path::new("/doesnotexist/testing.txt")),
            Err(VfsError::DirectoryDoesNotExist)
        );
        assert_eq!(
            ramfs.create_dir(Path::new("/doesnotexist/testing")),
            Err(VfsError::DirectoryDoesNotExist)
        );
    }

    #[test_case]
    fn delete_stuff() {
        let ramfs = Ramfs::new();
        let file = Path::new("/test.html");
        {
            let mut file = ramfs.create_file(file).unwrap();
            file.write(b"random_nonsense").unwrap();
        }
        ramfs.delete(file).unwrap();
        assert_eq!(ramfs.open_file(file), Err(VfsError::PathDoesNotExist));
        let folder = Path::new("/hello");
        {
            ramfs.create_dir(folder).unwrap();
            ramfs.create_file(Path::new("/hello/test.txt")).unwrap();
        }
        ramfs.delete(folder).unwrap();
        let Err(e) = ramfs.open_dir(folder) else {
            panic!("openning the folder worked")
        };
        assert_eq!(e, VfsError::PathDoesNotExist);
        assert_eq!(
            ramfs.open_file(Path::new("/hello/test.txt")),
            Err(VfsError::PathDoesNotExist)
        );
    }

    #[test_case]
    fn file_types() {
        let ramfs = Ramfs::new();
        let file = Path::new("/test.txt");
        ramfs.create_file(file).unwrap();
        assert_eq!(ramfs.file_type(file), Ok(FileType::File));
        assert_eq!(
            ramfs.file_type(Path::new("/some/random/path")),
            Err(VfsError::PathDoesNotExist)
        );
        let dir = Path::new("/hello");
        ramfs.create_dir(dir).unwrap();
        assert_eq!(ramfs.file_type(dir), Ok(FileType::Directory));
    }
}
