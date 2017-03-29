use ::prelude::*;

use std::path::Path;
use std::ffi::OsStr;
use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::mem;
use std::cmp::min;

use fuse;
use time::Timespec;
use libc;


macro_rules! fuse_try(
    ($val:expr, $reply:expr) => {
        match $val {
            Ok(val) => val,
            Err(err) => {
                info!("Error: {:?}", err);
                return $reply.error(libc::EIO);
            }
        }
    }
);

macro_rules! str(
    ($val:expr, $reply:expr) => {
        match $val.to_str() {
            Some(val) => val,
            None => {
                info!("Error: Name is not valid unicode");
                return $reply.error(libc::ENAMETOOLONG);
            }
        }
    }
);

macro_rules! inode(
    ($slf:expr, $num:expr, $reply:expr) => {
        match $slf.get_inode($num) {
            Some(inode) => inode,
            None => {
                info!("Error: Inode not found: {}", $num);
                return $reply.error(libc::EBADF)
            }
        }
    }
);

macro_rules! lookup(
    ($slf:expr, $parent:expr, $name:expr, $reply:expr) => {
        match fuse_try!($slf.get_child(&$parent, $name), $reply) {
            Some(inode) => inode,
            None => {
                info!("Error: Child node not found: {} -> {}", $parent.borrow().num, $name);
                return $reply.error(libc::ENOENT)
            }
        }
    }
);


#[inline]
fn convert_file_type(kind: FileType) -> fuse::FileType {
    match kind {
        FileType::Directory => fuse::FileType::Directory,
        FileType::File => fuse::FileType::RegularFile,
        FileType::Symlink => fuse::FileType::Symlink,
    }
}

type FuseInodeRef = Rc<RefCell<FuseInode>>;

pub struct FuseInode {
    num: u64,
    inode: Inode,
    parent: Option<FuseInodeRef>,
    children: HashMap<String, FuseInodeRef>,
    chunks: Option<ChunkList>
}

impl FuseInode {
    pub fn to_attrs(&self) -> fuse::FileAttr {
        fuse::FileAttr {
            ino: self.num,
            size: self.inode.size,
            blocks: self.inode.size / 512,
            atime: Timespec::new(self.inode.modify_time, 0),
            mtime: Timespec::new(self.inode.modify_time, 0),
            ctime: Timespec::new(0, 0),
            crtime: Timespec::new(0, 0),
            kind: convert_file_type(self.inode.file_type),
            perm: self.inode.mode as u16,
            nlink: 1,
            uid: self.inode.user,
            gid: self.inode.group,
            rdev: 0,
            flags: 0
        }
    }

    pub fn dir_list(&self) -> Option<Vec<(u64, fuse::FileType, String)>> {
        if self.inode.file_type != FileType::Directory {
            return None
        }
        let mut list = Vec::with_capacity(self.children.len()+2);
        list.push((self.num, fuse::FileType::Directory, ".".to_string()));
        if let Some(ref parent) = self.parent {
            let parent = parent.borrow();
            list.push((parent.num, fuse::FileType::Directory, "..".to_string()));
        } else {
            list.push((self.num, fuse::FileType::Directory, "..".to_string()));
        }
        for ch in self.children.values() {
            let child = ch.borrow();
            list.push((child.num, convert_file_type(child.inode.file_type), child.inode.name.clone()));
        }
        Some(list)
    }
}


pub struct FuseFilesystem<'a> {
    next_id: u64,
    repository: &'a mut Repository,
    inodes: HashMap<u64, FuseInodeRef>
}

impl<'a> FuseFilesystem<'a> {
    pub fn new(repository: &'a mut Repository) -> Result<Self, RepositoryError> {
        Ok(FuseFilesystem {
            next_id: 1,
            repository: repository,
            inodes: HashMap::new()
        })
    }

    pub fn from_repository(repository: &'a mut Repository) -> Result<Self, RepositoryError> {
        let mut backups = vec![];
        for (name, backup) in try!(repository.get_backups()) {
            let inode = try!(repository.get_inode(&backup.root));
            backups.push((name, inode));
        }
        let mut fs = try!(FuseFilesystem::new(repository));
        let root = fs.add_virtual_directory("".to_string(), None);
        for (name, mut backup) in backups {
            let mut parent = root.clone();
            for part in name.split('/') {
                parent = match fs.get_child(&parent, part).unwrap() {
                    Some(child) => child,
                    None => fs.add_virtual_directory(part.to_string(), Some(parent))
                };
            }
            let mut parent_mut = parent.borrow_mut();
            backup.name = parent_mut.inode.name.clone();
            parent_mut.inode = backup;
        }
        Ok(fs)
    }

    pub fn from_backup(repository: &'a mut Repository, backup: &Backup) -> Result<Self, RepositoryError> {
        let inode = try!(repository.get_inode(&backup.root));
        let mut fs = try!(FuseFilesystem::new(repository));
        fs.add_inode(inode, None);
        Ok(fs)
    }

    pub fn from_inode(repository: &'a mut Repository, inode: Inode) -> Result<Self, RepositoryError> {
        let mut fs = try!(FuseFilesystem::new(repository));
        fs.add_inode(inode, None);
        Ok(fs)
    }

    pub fn add_virtual_directory(&mut self, name: String, parent: Option<FuseInodeRef>) -> FuseInodeRef {
        self.add_inode(Inode {
            name: name,
            file_type: FileType::Directory,
            ..Default::default()
        }, parent)
    }

    pub fn add_inode(&mut self, inode: Inode, parent: Option<FuseInodeRef>) -> FuseInodeRef {
        let inode = FuseInode {
            inode: inode,
            num: self.next_id,
            parent: parent.clone(),
            chunks: None,
            children: HashMap::new()
        };
        let name = inode.inode.name.clone();
        let inode = Rc::new(RefCell::new(inode));
        self.inodes.insert(self.next_id, inode.clone());
        if let Some(parent) = parent {
            parent.borrow_mut().children.insert(name, inode.clone());
        }
        self.next_id += 1;
        inode
    }

    pub fn mount<P: AsRef<Path>>(self, mountpoint: P) -> Result<(), RepositoryError> {
        Ok(try!(fuse::mount(self, &mountpoint, &[
            OsStr::new("default_permissions"),
            OsStr::new("kernel_cache"),
            OsStr::new("auto_cache"),
            OsStr::new("readonly")
        ])))
    }

    pub fn get_inode(&mut self, num: u64) -> Option<FuseInodeRef> {
        self.inodes.get(&num).cloned()
    }

    pub fn get_child(&mut self, parent: &FuseInodeRef, name: &str) -> Result<Option<FuseInodeRef>, RepositoryError> {
        let mut parent_mut = parent.borrow_mut();
        if let Some(child) = parent_mut.children.get(name) {
            return Ok(Some(child.clone()))
        }
        let child;
        if let Some(chunks) = parent_mut.inode.children.as_ref().and_then(|c| c.get(name)) {
            child = Rc::new(RefCell::new(FuseInode {
                num: self.next_id,
                inode: try!(self.repository.get_inode(chunks)),
                parent: Some(parent.clone()),
                children: HashMap::new(),
                chunks: None
            }));
            self.inodes.insert(self.next_id, child.clone());
            self.next_id +=1;
        } else {
            return Ok(None)
        }
        parent_mut.children.insert(name.to_string(), child.clone());
        Ok(Some(child))
    }

    pub fn fetch_children(&mut self, parent: &FuseInodeRef) -> Result<(), RepositoryError> {
        let mut parent_mut = parent.borrow_mut();
        let mut parent_children = HashMap::new();
        mem::swap(&mut parent_children, &mut parent_mut.children);
        if let Some(ref children) = parent_mut.inode.children {
            for (name, chunks) in children {
                if !parent_mut.children.contains_key(name) {
                    let child = Rc::new(RefCell::new(FuseInode {
                        num: self.next_id,
                        inode: try!(self.repository.get_inode(chunks)),
                        parent: Some(parent.clone()),
                        children: HashMap::new(),
                        chunks: None
                    }));
                    self.inodes.insert(self.next_id, child.clone());
                    self.next_id +=1;
                    parent_children.insert(name.clone(), child);
                }
            }
        }
        mem::swap(&mut parent_children, &mut parent_mut.children);
        Ok(())
    }

    pub fn fetch_chunks(&mut self, inode: &FuseInodeRef) -> Result<(), RepositoryError> {
        let mut inode = inode.borrow_mut();
        let mut chunks = None;
        match inode.inode.contents {
            None | Some(FileContents::Inline(_)) => (),
            Some(FileContents::ChunkedDirect(ref c)) => {
                chunks = Some(c.clone());
            },
            Some(FileContents::ChunkedIndirect(ref c)) => {
                let chunk_data = try!(self.repository.get_data(c));
                chunks = Some(ChunkList::read_from(&chunk_data));
            }
        }
        inode.chunks = chunks;
        Ok(())
    }
}


impl<'a> fuse::Filesystem for FuseFilesystem<'a> {

    /// Look up a directory entry by name and get its attributes.
    fn lookup (&mut self, _req: &fuse::Request, parent: u64, name: &OsStr, reply: fuse::ReplyEntry) {
        let sname = str!(name, reply);
        let parent = inode!(self, parent, reply);
        let child = lookup!(self, &parent, sname, reply);
        let ttl = Timespec::new(60, 0);
        let attrs = child.borrow().to_attrs();
        reply.entry(&ttl, &attrs, 0)
    }

    fn destroy (&mut self, _req: &fuse::Request) {
        info!("destroy");
    }

    /// Forget about an inode
    /// The nlookup parameter indicates the number of lookups previously performed on
    /// this inode. If the filesystem implements inode lifetimes, it is recommended that
    /// inodes acquire a single reference on each lookup, and lose nlookup references on
    /// each forget. The filesystem may ignore forget calls, if the inodes don't need to
    /// have a limited lifetime. On unmount it is not guaranteed, that all referenced
    /// inodes will receive a forget message.
    fn forget (&mut self, _req: &fuse::Request, ino: u64, _nlookup: u64) {
        info!("forget {:?}", ino);
        //self.fs.forget(ino).unwrap();
    }

    /// Get file attributes
    fn getattr (&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyAttr) {
        let inode = inode!(self, ino, reply);
        let ttl = Timespec::new(60, 0);
        reply.attr(&ttl, &inode.borrow().to_attrs());
    }

    /// Set file attributes
    fn setattr (&mut self, _req: &fuse::Request, _ino: u64, _mode: Option<u32>, _uid: Option<u32>, _gid: Option<u32>, _size: Option<u64>, _atime: Option<Timespec>, _mtime: Option<Timespec>, _fh: Option<u64>, _crtime: Option<Timespec>, _chgtime: Option<Timespec>, _bkuptime: Option<Timespec>, _flags: Option<u32>, reply: fuse::ReplyAttr) {
        reply.error(libc::EROFS)
    }

    /// Read symbolic link
    fn readlink (&mut self, _req: &fuse::Request, ino: u64, reply: fuse::ReplyData) {
        let inode = inode!(self, ino, reply);
        let inode = inode.borrow();
        match inode.inode.symlink_target {
            None => reply.error(libc::EINVAL),
            Some(ref link) => reply.data(link.as_bytes())
        }
    }

    /// Create a hard link
    fn link (&mut self, _req: &fuse::Request, _ino: u64, _newparent: u64, _newname: &OsStr, reply: fuse::ReplyEntry) {
        reply.error(libc::EROFS)
    }

    /// Create file node
    /// Create a regular file, character device, block device, fifo or socket node.
    fn mknod (&mut self, _req: &fuse::Request, _parent: u64, _name: &OsStr, _mode: u32, _rdev: u32, reply: fuse::ReplyEntry) {
        reply.error(libc::EROFS)
    }

    /// Create a directory
    fn mkdir (&mut self, _req: &fuse::Request, _parent: u64, _name: &OsStr, _mode: u32, reply: fuse::ReplyEntry) {
        reply.error(libc::EROFS)
    }

    /// Remove a file
    fn unlink (&mut self, _req: &fuse::Request, _parent: u64, _name: &OsStr, reply: fuse::ReplyEmpty) {
        reply.error(libc::EROFS)
    }

    /// Remove a directory
    fn rmdir (&mut self, _req: &fuse::Request, _parent: u64, _name: &OsStr, reply: fuse::ReplyEmpty) {
        reply.error(libc::EROFS)
    }

    /// Create a symbolic link
    fn symlink (&mut self, _req: &fuse::Request, _parent: u64, _name: &OsStr, _link: &Path, reply: fuse::ReplyEntry) {
        reply.error(libc::EROFS)
    }

    /// Rename a file
    fn rename (&mut self, _req: &fuse::Request, _parent: u64, _name: &OsStr, _newparent: u64, _newname: &OsStr, reply: fuse::ReplyEmpty) {
        reply.error(libc::EROFS)
    }

    /// Open a file
    /// Open flags (with the exception of O_CREAT, O_EXCL, O_NOCTTY and O_TRUNC) are
    /// available in flags. Filesystem may store an arbitrary file handle (pointer, index,
    /// etc) in fh, and use this in other all other file operations (read, write, flush,
    /// release, fsync). Filesystem may also implement stateless file I/O and not store
    /// anything in fh. There are also some flags (direct_io, keep_cache) which the
    /// filesystem may set, to change the way the file is opened. See fuse_file_info
    /// structure in <fuse_common.h> for more details.
    fn open (&mut self, _req: &fuse::Request, ino: u64, flags: u32, reply: fuse::ReplyOpen) {
        info!("open {:?}, flags: {:o}", ino, flags);
        if (flags & (libc::O_WRONLY | libc::O_RDWR | libc::O_TRUNC) as u32) != 0 {
            return reply.error(libc::EROFS);
        }
        let inode = inode!(self, ino, reply);
        fuse_try!(self.fetch_chunks(&inode), reply);
        reply.opened(ino, libc::O_RDONLY as u32);
    }

    /// Read data
    /// Read should send exactly the number of bytes requested except on EOF or error,
    /// otherwise the rest of the data will be substituted with zeroes. An exception to
    /// this is when the file has been opened in 'direct_io' mode, in which case the
    /// return value of the read system call will reflect the return value of this
    /// operation. fh will contain the value set by the open method, or will be undefined
    /// if the open method didn't set any value.
    fn read (&mut self, _req: &fuse::Request, ino: u64, _fh: u64, mut offset: u64, mut size: u32, reply: fuse::ReplyData) {
        info!("read {:?}, offset {}, size {}", ino, offset, size);
        let inode = inode!(self, ino, reply);
        let inode = inode.borrow();
        match inode.inode.contents {
            None => return reply.data(&[]),
            Some(FileContents::Inline(ref data)) => return reply.data(&data[min(offset as usize, data.len())..min(offset as usize+size as usize, data.len())]),
            _ => ()
        }
        if let Some(ref chunks) = inode.chunks {
            let mut data = Vec::with_capacity(size as usize);
            for &(hash, len) in chunks.iter() {
                if len as u64 <= offset {
                    offset -= len as u64;
                    continue
                }
                let chunk = match fuse_try!(self.repository.get_chunk(hash), reply) {
                    Some(chunk) => chunk,
                    None => return reply.error(libc::EIO)
                };
                assert_eq!(chunk.len() as u32, len);
                data.extend_from_slice(&chunk[offset as usize..min(offset as usize + size as usize, len as usize)]);
                if len - offset as u32 >= size {
                    break
                }
                size -= len - offset as u32;
                offset = 0;
            }
            reply.data(&data)
        } else {
            reply.error(libc::EBADF)
        }
    }

    /// Write data
    fn write (&mut self, _req: &fuse::Request, _ino: u64, _fh: u64, _offset: u64, _data: &[u8], _flags: u32, reply: fuse::ReplyWrite) {
        reply.error(libc::EROFS)
    }

    /// Flush method
    fn flush (&mut self, _req: &fuse::Request, _ino: u64, _fh: u64, _lock_owner: u64, reply: fuse::ReplyEmpty) {
        reply.ok()
    }

    /// Release an open file
    /// Release is called when there are no more references to an open file: all file
    /// descriptors are closed and all memory mappings are unmapped. For every open
    /// call there will be exactly one release call. The filesystem may reply with an
    /// error, but error values are not returned to close() or munmap() which triggered
    /// the release. fh will contain the value set by the open method, or will be undefined
    /// if the open method didn't set any value. flags will contain the same flags as for
    /// open.
    fn release (&mut self, _req: &fuse::Request, _ino: u64, _fh: u64, _flags: u32, _lock_owner: u64, _flush: bool, reply: fuse::ReplyEmpty) {
        /*if self.read_fds.remove(&fh).is_some() || self.write_fds.remove(&fh).is_some() {
            reply.ok();
        } else {
            reply.error(libc::EBADF);
        }*/
        reply.error(libc::ENOSYS)
    }

    /// Synchronize file contents
    fn fsync (&mut self, _req: &fuse::Request, _ino: u64, _fh: u64, _datasync: bool, reply: fuse::ReplyEmpty) {
        reply.ok()
    }

    /// Open a directory, finished
    fn opendir (&mut self, _req: &fuse::Request, ino: u64, _flags: u32, reply: fuse::ReplyOpen) {
        let dir = inode!(self, ino, reply);
        fuse_try!(self.fetch_children(&dir), reply);
        reply.opened(ino, 0);
    }

    /// Read directory, finished
    fn readdir (&mut self, _req: &fuse::Request, ino: u64, _fh: u64, offset: u64, mut reply: fuse::ReplyDirectory) {
        let dir = inode!(self, ino, reply);
        let dir = dir.borrow();
        if let Some(entries) = dir.dir_list() {
            for (i, (num, file_type, name)) in entries.into_iter().enumerate() {
                if i < offset as usize {
                    continue
                }
                if reply.add(num, i as u64 +1, file_type, &Path::new(&name)) {
                    break
                }
            }
            reply.ok()
        } else {
            reply.error(libc::ENOTDIR)
        }
    }

    /// Release an open directory, finished
    fn releasedir (&mut self, _req: &fuse::Request, _ino: u64, _fh: u64, _flags: u32, reply: fuse::ReplyEmpty) {
        reply.ok()
    }

    /// Synchronize directory contents, finished
    fn fsyncdir (&mut self, _req: &fuse::Request, _ino: u64, _fh: u64, _datasync: bool, reply: fuse::ReplyEmpty) {
        reply.ok()
    }

    /// Get file system statistics
    fn statfs (&mut self, _req: &fuse::Request, _ino: u64, reply: fuse::ReplyStatfs) {
        let info = self.repository.info();
        reply.statfs(
            info.raw_data_size/512 as u64, //total blocks
            0, //free blocks for admin
            0, //free blocks for users
            0,
            0,
            512 as u32, //block size
            255, //max name length
            0
        );
    }

    /// Set an extended attribute
    fn setxattr (&mut self, _req: &fuse::Request, _ino: u64, _name: &OsStr, _value: &[u8], _flags: u32, _position: u32, reply: fuse::ReplyEmpty) {
        reply.error(libc::EROFS)
    }

    /// Get an extended attribute
    fn getxattr (&mut self, _req: &fuse::Request, _ino: u64, _name: &OsStr, _size: u32, reply: fuse::ReplyXattr) {
        // #FIXME:30 If arg.size is zero, the size of the value should be sent with fuse_getxattr_out
        // #FIXME:0 If arg.size is non-zero, send the value if it fits, or ERANGE otherwise
        reply.error(libc::ENOSYS);
    }

    /// List extended attribute names
    fn listxattr (&mut self, _req: &fuse::Request, _ino: u64, _size: u32, reply: fuse::ReplyXattr) {
        // #FIXME:20 If arg.size is zero, the size of the attribute list should be sent with fuse_getxattr_out
        // #FIXME:10 If arg.size is non-zero, send the attribute list if it fits, or ERANGE otherwise
        reply.error(libc::ENOSYS);
    }

    /// Remove an extended attribute
    fn removexattr (&mut self, _req: &fuse::Request, _ino: u64, _name: &OsStr, reply: fuse::ReplyEmpty) {
        reply.error(libc::EROFS)
    }

    /// Check file access permissions
    /// This will be called for the access() system call. If the 'default_permissions'
    /// mount option is given, this method is not called. This method is not called
    /// under Linux kernel versions 2.4.x
    fn access (&mut self, _req: &fuse::Request, _ino: u64, _mask: u32, reply: fuse::ReplyEmpty) {
        reply.error(libc::ENOSYS);
    }

    /// Create and open a file
    fn create (&mut self, _req: &fuse::Request, _parent: u64, _name: &OsStr, _mode: u32, _flags: u32, reply: fuse::ReplyCreate) {
        reply.error(libc::EROFS)
    }

    /// Test for a POSIX file lock
    fn getlk (&mut self, _req: &fuse::Request, _ino: u64, _fh: u64, _lock_owner: u64, _start: u64, _end: u64, _typ: u32, _pid: u32, reply: fuse::ReplyLock) {
        reply.error(libc::ENOSYS);
    }

    /// Acquire, modify or release a POSIX file lock
    fn setlk (&mut self, _req: &fuse::Request, _ino: u64, _fh: u64, _lock_owner: u64, _start: u64, _end: u64, _typ: u32, _pid: u32, _sleep: bool, reply: fuse::ReplyEmpty) {
        reply.error(libc::ENOSYS);
    }

    /// Map block index within file to block index within device
    fn bmap (&mut self, _req: &fuse::Request, _ino: u64, _blocksize: u32, _idx: u64, reply: fuse::ReplyBmap) {
        reply.error(libc::ENOSYS);
    }

}
