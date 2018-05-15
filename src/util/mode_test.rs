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


pub enum RepositoryError {
    Error
}

enum LockMode {
    None, Shared, Exclusive
}



struct RepositoryInner {
}


impl RepositoryInner {
    fn set_local_lock(&mut self, mode: LockMode) -> Result<(), RepositoryError> {
        Ok(())
    }

    fn set_remote_lock(&mut self, mode: LockMode) -> Result<(), RepositoryError> {
        Ok(())
    }

    fn set_dirty(&mut self, dirty: bool) -> Result<(), RepositoryError> {
        Ok(())
    }

}

pub trait ReadonlyMode {
    fn func1(&self) -> Result<(), RepositoryError>;
}

impl ReadonlyMode for RepositoryInner {
    fn func1(&self) -> Result<(), RepositoryError> {
        Ok(())
    }
}


pub trait LocalWriteMode: ReadonlyMode {
    fn func2(&self) -> Result<(), RepositoryError>;
}

impl LocalWriteMode for RepositoryInner {
    fn func2(&self) -> Result<(), RepositoryError> {
        Ok(())
    }
}


pub trait OnlineMode: LocalWriteMode {

}

impl OnlineMode for RepositoryInner {

}


pub trait BackupMode: OnlineMode {

}

impl BackupMode for RepositoryInner {

}


pub trait VacuumMode: BackupMode {

}

impl VacuumMode for RepositoryInner {

}



pub trait UpgradeToLocalWriteMode {
    fn in_local_write_mode<R, E: From<RepositoryError>, F: FnOnce(&mut LocalWriteMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E>;
}

impl UpgradeToLocalWriteMode for RepositoryInner {
    fn in_local_write_mode<R, E: From<RepositoryError>, F: FnOnce(&mut LocalWriteMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E> {
        try!(self.set_local_lock(LockMode::Exclusive));
        try!(self.set_dirty(true));
        let res = f(self);
        if res.is_ok() {
            try!(self.set_dirty(false));
        }
        try!(self.set_local_lock(LockMode::Shared));
        res
    }
}


pub trait UpgradeToOnlineMode {
    fn in_online_mode<R, E: From<RepositoryError>, F: FnOnce(&mut OnlineMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E>;
}

impl UpgradeToOnlineMode for RepositoryInner {
    fn in_online_mode<R, E: From<RepositoryError>, F: FnOnce(&mut OnlineMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E> {
        try!(self.set_local_lock(LockMode::Exclusive));
        try!(self.set_remote_lock(LockMode::Shared));
        try!(self.set_dirty(true));
        let res = f(self);
        if res.is_ok() {
            try!(self.set_dirty(false));
        }
        try!(self.set_remote_lock(LockMode::None));
        try!(self.set_local_lock(LockMode::Shared));
        res
    }
}


pub trait UpgradeToBackupMode {
    fn in_backup_mode<R, E: From<RepositoryError>, F: FnOnce(&mut BackupMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E>;
}

impl UpgradeToBackupMode for RepositoryInner {
    fn in_backup_mode<R, E: From<RepositoryError>, F: FnOnce(&mut BackupMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E> {
        try!(self.set_local_lock(LockMode::Exclusive));
        try!(self.set_remote_lock(LockMode::Shared));
        try!(self.set_dirty(true));
        let res = f(self);
        if res.is_ok() {
            try!(self.set_dirty(false));
        }
        try!(self.set_remote_lock(LockMode::None));
        try!(self.set_local_lock(LockMode::Shared));
        res
    }
}


pub trait UpgradeToVacuumMode {
    fn in_vacuum_mode<R, E: From<RepositoryError>, F: FnOnce(&mut VacuumMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E>;
}

impl UpgradeToVacuumMode for RepositoryInner {
    fn in_vacuum_mode<R, E: From<RepositoryError>, F: FnOnce(&mut VacuumMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E> {
        try!(self.set_local_lock(LockMode::Exclusive));
        try!(self.set_remote_lock(LockMode::Exclusive));
        try!(self.set_dirty(true));
        let res = f(self);
        if res.is_ok() {
            try!(self.set_dirty(false));
        }
        try!(self.set_remote_lock(LockMode::None));
        try!(self.set_local_lock(LockMode::Shared));
        res
    }
}


pub struct Repository(RepositoryInner);

impl ReadonlyMode for Repository {
    fn func1(&self) -> Result<(), RepositoryError> {
        self.0.func1()
    }
}

impl UpgradeToLocalWriteMode for Repository {
    fn in_local_write_mode<R, E: From<RepositoryError>, F: FnOnce(&mut LocalWriteMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E> {
        self.0.in_local_write_mode(f)
    }
}

impl UpgradeToOnlineMode for Repository {
    fn in_online_mode<R, E: From<RepositoryError>, F: FnOnce(&mut OnlineMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E> {
        self.0.in_online_mode(f)
    }
}

impl UpgradeToBackupMode for Repository {
    fn in_backup_mode<R, E: From<RepositoryError>, F: FnOnce(&mut BackupMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E> {
        self.0.in_backup_mode(f)
    }
}

impl UpgradeToVacuumMode for Repository {
    fn in_vacuum_mode<R, E: From<RepositoryError>, F: FnOnce(&mut VacuumMode) -> Result<R, E>>(&mut self, f: F) -> Result<R, E> {
        self.0.in_vacuum_mode(f)
    }
}


impl Repository {

}


fn test_it(mut repo: Repository) {
    repo.func1();
    repo.in_local_write_mode(|repo| repo.func2());
}