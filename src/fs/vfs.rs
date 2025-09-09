use alloc::vec::Vec;

use super::path::Path;
use crate::{alloc::boxed::Box, fs::path::PathBuf};
use spin::RwLock;
pub type Result<T> = core::result::Result<T, VfsError>;

#[derive(Debug, PartialEq)]
pub enum VfsError {
    /// Path doesn't exist. Should be thrown in FileSystem::open_file or FileSystem::delete
    /// or FileSystem::open_dir or FileSystem::file_type
    PathDoesNotExist,
    /// The given directory in the path does not exist. Should be thrown in FileSystem::create_file or FileSystem::create_dir,
    /// when the given paths have a directory in them which do not exist.
    DirectoryDoesNotExist,
    /// The read has failed due to some reason. Should be thrown in File::read and similar.
    ReadFailed,
    /// The write has failed due to some reason. Should be thrown in File::write and similar.
    WriteFailed,
    /// The given path already exists. Should be thrown in FileSystem::create_file or FileSystem::create_dir.
    PathAlreadyExists,
    /// The given path is not absolute. Should be thrown in all of the FileSystem:: api when the path is not absolute.
    PathIsNotAbsolute,
    /// The given path does not have a filename. Should be thrown in FileSystem::open_file and FileSystem::create_file.
    PathDoesNotHaveAFilename,
}
pub trait File {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn write(&mut self, buf: &[u8]) -> Result<usize>;
}

#[derive(Debug, PartialEq)]
pub enum FileType {
    File,
    Directory,
}
pub struct DirEntry {
    pub file_type: FileType,
    pub path: PathBuf,
}
pub trait FileSystem {
    type File;

    /// open = get a refrence to the file.
    fn open_file(&self, path: &Path) -> Result<Self::File>;
    fn open_dir(&self, path: &Path) -> Result<Box<dyn Iterator<Item = DirEntry>>>;
    fn file_type(&self, path: &Path) -> Result<FileType>;
    fn delete(&self, path: &Path) -> Result<()>;
    fn create_file(&self, path: &Path) -> Result<Self::File>;
    fn create_dir(&self, path: &Path) -> Result<()>;

    fn exists(&self, path: &Path) -> bool {
        self.file_type(path).is_ok()
    }
}

pub type DynFileSystem = Box<dyn FileSystem<File = Box<dyn File>>>;
struct Mount {
    path: PathBuf,
    filesystem: Box<dyn FileSystem<File = Box<dyn File>>>,
}

pub struct Vfs {
    root: Box<dyn FileSystem<File = Box<dyn File>>>,
    mounts: RwLock<Vec<Mount>>,
}

impl Vfs {
    pub fn mount(&self, fs: DynFileSystem, path: &Path) -> Result<()> {
        if !path.has_root() {
            return Err(VfsError::PathIsNotAbsolute);
        }
        if self.root.exists(path) {
            return Err(VfsError::PathAlreadyExists);
        }
        let mut mounts = self.mounts.write();
        if mounts.iter().find(|m| m.path.as_path() == path).is_some() {
            Err(VfsError::PathAlreadyExists)
        } else {
            let mount = Mount {
                path: PathBuf::from(path),
                filesystem: fs,
            };
            mounts.push(mount);
            Ok(())
        }
    }

    fn open_file_in_mounts(&self, path: &Path) -> Result<Box<dyn File>> {
        for mount in self.mounts.read().iter() {
            if let Some(path) = path.relative_to(&mount.path) {
                return mount.filesystem.open_file(path);
            }
        }
        Err(VfsError::PathDoesNotExist)
    }
}

impl FileSystem for Vfs {
    type File = Box<dyn File>;

    fn file_type(&self, path: &Path) -> Result<FileType> {
        todo!()
    }
    fn open_file(&self, path: &Path) -> Result<Self::File> {
        if !path.has_root() {
            return Err(VfsError::PathIsNotAbsolute);
        }
        self.root.open_file(path).or(self.open_file_in_mounts(path))
    }

    fn create_file(&self, path: &Path) -> Result<Self::File> {
        todo!()
    }

    fn create_dir(&self, path: &Path) -> Result<()> {
        todo!()
    }

    fn delete(&self, path: &Path) -> Result<()> {
        todo!()
    }

    fn open_dir(&self, path: &Path) -> Result<Box<dyn Iterator<Item = DirEntry>>> {
        todo!()
    }
}
