use ::prelude::*;

pub struct Repository(RepositoryInner);
pub struct LocalWriteMode<'a>(&'a mut RepositoryInner);
pub struct RestoreMode<'a>(&'a mut RepositoryInner);
pub struct BackupMode<'a>(&'a mut RepositoryInner);
pub struct VacuumMode<'a>(&'a mut RepositoryInner);

macro_rules! in_readonly_mode {
    ( $($f:tt)* ) => {
        impl Repository {
            $( $f )*
        }
        impl<'a> LocalWriteMode<'a> {
            $( $f )*
        }
        impl<'a> RestoreMode<'a> {
            $( $f )*
        }
        impl<'a> BackupMode<'a> {
            $( $f )*
        }
        impl<'a> VacuumMode<'a> {
            $( $f )*
        }
    };
}

macro_rules! in_local_write_mode {
    ( $($f:tt)* ) => {
        impl<'a> LocalWriteMode<'a> {
            $( $f )*
        }
        impl<'a> RestoreMode<'a> {
            $( $f )*
        }
        impl<'a> BackupMode<'a> {
            $( $f )*
        }
        impl<'a> VacuumMode<'a> {
            $( $f )*
        }
    };
}

macro_rules! in_restore_mode {
    ( $($f:tt)* ) => {
        impl<'a> RestoreMode<'a> {
            $( $f )*
        }
        impl<'a> BackupMode<'a> {
            $( $f )*
        }
        impl<'a> VacuumMode<'a> {
            $( $f )*
        }
    };
}

macro_rules! in_backup_mode {
    ( $($f:tt)* ) => {
        impl<'a> BackupMode<'a> {
            $( $f )*
        }
        impl<'a> VacuumMode<'a> {
            $( $f )*
        }
    };
}

macro_rules! in_vacuum_mode {
    ( $($f:tt)* ) => {
        impl<'a> VacuumMode<'a> {
            $( $f )*
        }
    };
}


impl RepositoryInner {
    fn local_write_mode<R, F: FnOnce(LocalWriteMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        let ret = f(LocalWriteMode(self));
        ret
    }

    fn restore_mode<R, F: FnOnce(RestoreMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        let ret = f(RestoreMode(self));
        ret
    }

    fn backup_mode<R, F: FnOnce(BackupMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        let ret = f(BackupMode(self));
        ret
    }

    fn vacuum_mode<R, F: FnOnce(VacuumMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        let ret = f(VacuumMode(self));
        ret
    }
}


impl Repository {
    pub fn local_write_mode<R, F: FnOnce(LocalWriteMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.local_write_mode(f)
    }

    pub fn restore_mode<R, F: FnOnce(RestoreMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.restore_mode(f)
    }

    pub fn backup_mode<R, F: FnOnce(BackupMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.backup_mode(f)
    }

    pub fn vacuum_mode<R, F: FnOnce(VacuumMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.vacuum_mode(f)
    }
}

impl<'a> LocalWriteMode<'a> {
    pub fn restore_mode<R, F: FnOnce(RestoreMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.restore_mode(f)
    }

    pub fn backup_mode<R, F: FnOnce(BackupMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.backup_mode(f)
    }

    pub fn vacuum_mode<R, F: FnOnce(VacuumMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.vacuum_mode(f)
    }
}

impl<'a> RestoreMode<'a> {
    pub fn backup_mode<R, F: FnOnce(BackupMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.backup_mode(f)
    }

    pub fn vacuum_mode<R, F: FnOnce(VacuumMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.vacuum_mode(f)
    }
}

impl<'a> BackupMode<'a> {
    pub fn vacuum_mode<R, F: FnOnce(VacuumMode) -> Result<R, RepositoryError>>(&mut self, f: F) -> Result<R, RepositoryError> {
        self.0.vacuum_mode(f)
    }
}


impl Repository {
    fn test(&mut self) {
        self.local_write_mode(|s| {
            s.dummy("aaa");
            Ok(())
        });
    }
}

in_readonly_mode! {
    pub fn get_config(&self) -> &Config {
        self.0.get_config()
    }

    pub fn set_config(&mut self, config: Config) {
        self.0.set_config(config);
    }
}


in_local_write_mode! {
    fn dummy<R>(&self, r: R) {

    }
}


