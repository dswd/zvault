/**

ReadonlyMode
- Local: readonly, shared lock
- Remote: offline

LocalWriteMode
- Local: writable, exclusive lock, dirty flag
- Remote: offline

OnlineMode
- Local: writable, exclusive lock, dirty flag
- Remote: readonly, shared lock

BackupMode
- Local: writable, exclusive lock, dirty flag
- Remote: append-only, shared lock

VacuumMode
- Local: writable, exclusive lock, dirty flag
- Remote: writable, exclusive lock

**/


pub struct Repository {

}

impl Repository {
    pub fn readonly_mode<R, F: FnOnce(&mut Repository, &ReadonlyMode) -> R> (&mut self, f: F) -> R {
        f(self, &Lock)
    }

    pub fn localwrite_mode<R, F: FnOnce(&mut Repository, &LocalWriteMode) -> R> (&mut self, f: F) -> R {
        f(self, &Lock)
    }

    pub fn online_mode<R, F: FnOnce(&mut Repository, &OnlineMode) -> R> (&mut self, f: F) -> R {
        f(self, &Lock)
    }

    pub fn backup_mode<R, F: FnOnce(&mut Repository, &BackupMode) -> R> (&mut self, f: F) -> R {
        f(self, &Lock)
    }

    pub fn vacuum_mode<R, F: FnOnce(&mut Repository, &VacuumMode) -> R> (&mut self, f: F) -> R {
        f(self, &Lock)
    }
}


struct Lock;

pub trait ReadonlyMode {}

impl ReadonlyMode for Lock {}


pub trait LocalWriteMode: ReadonlyMode {
    fn as_readonly(&self) -> &ReadonlyMode;
}

impl LocalWriteMode for Lock {
    fn as_readonly(&self) -> &ReadonlyMode {
        self
    }
}


pub trait OnlineMode: LocalWriteMode {
    fn as_localwrite(&self) -> &LocalWriteMode;
}

impl OnlineMode for Lock {
    fn as_localwrite(&self) -> &LocalWriteMode {
        self
    }
}


pub trait BackupMode: OnlineMode {
    fn as_online(&self) -> &OnlineMode;
}

impl BackupMode for Lock {
    fn as_online(&self) -> &OnlineMode {
        self
    }
}


pub trait VacuumMode: BackupMode {
    fn as_backup(&self) -> &BackupMode;
}

impl VacuumMode for Lock {
    fn as_backup(&self) -> &BackupMode {
        self
    }
}


impl Repository {
    fn write<W: ::std::io::Write>(&mut self, w: W, lock: &LocalWriteMode) {

    }

    fn test(&mut self) {
        self.localwrite_mode(|repo, lock| {
            repo.write(&mut Vec::new(), lock)
        });
        self.online_mode(|repo, lock| {
            repo.write(&mut Vec::new(), lock.as_localwrite())
        });
    }
}