use libc;
use std::ffi;

extern {
    fn gethostname(name: *mut libc::c_char, size: libc::size_t) -> libc::c_int;
}

pub fn get_hostname() -> Result<String, ()> {
    let mut buf = Vec::with_capacity(255);
    buf.resize(255, 0u8);
    if unsafe { gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len() as libc::size_t) } == 0 {
        buf[254] = 0; //enforce null-termination
        let name = unsafe { ffi::CStr::from_ptr(buf.as_ptr() as *const libc::c_char) };
        name.to_str().map(|s| s.to_string()).map_err(|_| ())
    } else {
        Err(())
    }
}



mod tests {

    #[allow(unused_imports)]
    use super::*;


    #[test]
    fn test_gethostname() {
        let res = get_hostname();
        assert!(res.is_ok());
        let name = res.unwrap();
        assert!(name.len() >= 1);
    }

}
