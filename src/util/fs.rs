mod linux {
    use libc;

    use std::path::Path;
    use std::io;
    use std::ffi::CString;
    use std::os::unix::ffi::OsStringExt;

    pub fn chown<P: AsRef<Path>>(path: P, uid: libc::uid_t, gid: libc::gid_t) -> Result<(), io::Error> {
        let path = CString::new(path.as_ref().to_path_buf().into_os_string().into_vec()).unwrap();
        let result = unsafe { libc::lchown((&path).as_ptr(), uid, gid) };
        match result {
            0 => Ok(()),
            err => Err(io::Error::from_raw_os_error(err))
        }
    }
}

pub use self::linux::*;
