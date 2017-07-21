use prelude::*;

use serde_yaml;
use chrono::prelude::*;
use libc;

use std::path::{Path, PathBuf};
use std::io;
use std::fs::{self, File};


quick_error!{
    #[derive(Debug)]
    pub enum LockError {
        Io(err: io::Error) {
            from()
            cause(err)
            description("IO error")
            display("Lock error: IO error\n\tcaused by: {}", err)
        }
        Yaml(err: serde_yaml::Error) {
            from()
            cause(err)
            description("Yaml format error")
            display("Lock error: yaml format error\n\tcaused by: {}", err)
        }
        InvalidLockState(reason: &'static str) {
            description("Invalid lock state")
            display("Lock error: invalid lock state: {}", reason)
        }
        Locked {
            description("Locked")
            display("Lock error: locked")
        }
    }
}


#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct LockFile {
    pub hostname: String,
    pub processid: usize,
    pub date: i64,
    pub exclusive: bool
}
serde_impl!(LockFile(String) {
    hostname: String => "hostname",
    processid: usize => "processid",
    date: i64 => "date",
    exclusive: bool => "exclusive"
});

impl LockFile {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, LockError> {
        let f = try!(File::open(path));
        Ok(try!(serde_yaml::from_reader(f)))
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), LockError> {
        let mut f = try!(File::create(path));
        Ok(try!(serde_yaml::to_writer(&mut f, &self)))
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum LockLevel {
    Free,
    Shared,
    Exclusive
}


pub struct LockHandle {
    lock: LockFile,
    path: PathBuf
}

impl LockHandle {
    pub fn release(&self) -> Result<(), LockError> {
        if self.path.exists() {
            try!(fs::remove_file(&self.path))
        }
        Ok(())
    }

    pub fn refresh(&self) -> Result<(), LockError> {
        let mut file = try!(LockFile::load(&self.path));
        file.date = Utc::now().timestamp();
        file.save(&self.path)
    }
}

impl Drop for LockHandle {
    fn drop(&mut self) {
        self.release().unwrap()
    }
}



pub struct LockFolder {
    path: PathBuf
}

impl LockFolder {
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        LockFolder { path: path.as_ref().to_path_buf() }
    }

    fn get_locks(&self) -> Result<Vec<LockFile>, LockError> {
        let mut locks = vec![];
        for entry in try!(fs::read_dir(&self.path)) {
            let entry = try!(entry);
            locks.push(try!(LockFile::load(entry.path())));
        }
        Ok(locks)
    }

    pub fn get_lock_level(&self) -> Result<LockLevel, LockError> {
        let mut level = LockLevel::Free;
        for lock in try!(self.get_locks()) {
            if lock.exclusive {
                if level == LockLevel::Exclusive {
                    return Err(LockError::InvalidLockState("multiple exclusive locks"));
                } else {
                    level = LockLevel::Exclusive
                }
            } else if level == LockLevel::Exclusive {
                return Err(LockError::InvalidLockState(
                    "exclusive lock and shared locks"
                ));
            } else {
                level = LockLevel::Shared
            }
        }
        Ok(level)
    }

    pub fn lock(&self, exclusive: bool) -> Result<LockHandle, LockError> {
        let level = try!(self.get_lock_level());
        if level == LockLevel::Exclusive || level == LockLevel::Shared && exclusive {
            return Err(LockError::Locked);
        }
        let lockfile = LockFile {
            hostname: get_hostname().unwrap(),
            processid: unsafe { libc::getpid() } as usize,
            date: Utc::now().timestamp(),
            exclusive: exclusive
        };
        let path = self.path.join(format!(
            "{}-{}.lock",
            &lockfile.hostname,
            lockfile.processid
        ));
        try!(lockfile.save(&path));
        let handle = LockHandle {
            lock: lockfile,
            path: path
        };
        if self.get_lock_level().is_err() {
            try!(handle.release());
            return Err(LockError::Locked);
        }
        Ok(handle)
    }

    pub fn upgrade(&self, lock: &mut LockHandle) -> Result<(), LockError> {
        let lockfile = &mut lock.lock;
        if lockfile.exclusive {
            return Ok(());
        }
        let level = try!(self.get_lock_level());
        if level == LockLevel::Exclusive {
            return Err(LockError::Locked);
        }
        lockfile.exclusive = true;
        let path = self.path.join(format!(
            "{}-{}.lock",
            &lockfile.hostname,
            lockfile.processid
        ));
        try!(lockfile.save(&path));
        if self.get_lock_level().is_err() {
            lockfile.exclusive = false;
            try!(lockfile.save(&path));
            return Err(LockError::Locked);
        }
        Ok(())
    }

    pub fn downgrade(&self, lock: &mut LockHandle) -> Result<(), LockError> {
        let lockfile = &mut lock.lock;
        if !lockfile.exclusive {
            return Ok(());
        }
        lockfile.exclusive = false;
        let path = self.path.join(format!(
            "{}-{}.lock",
            &lockfile.hostname,
            lockfile.processid
        ));
        lockfile.save(&path)
    }
}
