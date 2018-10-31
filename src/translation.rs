use std::borrow::Cow;
use std::collections::HashMap;

use std::cmp::max;
use std::str;

use std::path::{Path, PathBuf};
use std::io::Read;
use std::fs::File;

use locale_config::Locale;


pub type CowStr = Cow<'static, str>;

fn read_u32(b: &[u8], reorder: bool) -> u32 {
    if reorder {
        (u32::from(b[0]) << 24) + (u32::from(b[1]) << 16) + (u32::from(b[2]) << 8) + u32::from(b[3])
    } else {
        (u32::from(b[3]) << 24) + (u32::from(b[2]) << 16) + (u32::from(b[1]) << 8) + u32::from(b[0])
    }
}

struct MoFile<'a> {
    data: &'a [u8],
    count: usize,
    orig_pos: usize,
    trans_pos: usize,
    reorder: bool,
    i : usize
}

impl<'a> MoFile<'a> {
    fn new_file(data: &'a [u8]) -> Result<Self, ()> {
        if data.len() < 20 {
            return Err(());
        }
        // Magic header
        let magic = read_u32(&data[0..4], false);
        let reorder = if magic == 0x9504_12de {
            false
        } else if magic == 0xde12_0495 {
            true
        } else {
            return Err(());
        };
        // Version
        if read_u32(&data[4..8], reorder) != 0x0000_0000 {
            return Err(());
        }
        // Translation count
        let count = read_u32(&data[8..12], reorder) as usize;
        // Original string offset
        let orig_pos = read_u32(&data[12..16], reorder) as usize;
        // Original string offset
        let trans_pos = read_u32(&data[16..20], reorder) as usize;
        if data.len() < max(orig_pos, trans_pos) + count * 8 {
            return Err(());
        }
        Ok(MoFile{
            data,
            count,
            orig_pos,
            trans_pos,
            reorder,
            i: 0
        })
    }
}

impl<'a> Iterator for MoFile<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        if self.i >= self.count {
            return None;
        }
        let length = read_u32(&self.data[self.orig_pos+self.i*8..], self.reorder) as usize;
        let offset = read_u32(&self.data[self.orig_pos+self.i*8+4..], self.reorder) as usize;
        let orig = match str::from_utf8(&self.data[offset..offset+length]) {
            Ok(s) => s,
            Err(_) => return None
        };
        let length = read_u32(&self.data[self.trans_pos+self.i*8..], self.reorder) as usize;
        let offset = read_u32(&self.data[self.trans_pos+self.i*8+4..], self.reorder) as usize;
        let trans = match str::from_utf8(&self.data[offset..offset+length]) {
            Ok(s) => s,
            Err(_) => return None
        };
        self.i += 1;
        Some((orig, trans))
    }
}


pub struct Translation(HashMap<CowStr, CowStr>);

impl Translation {
    pub fn new() -> Self {
        Translation(Default::default())
    }

    pub fn from_mo_data(data: &'static[u8]) -> Self {
        let mut translation = Translation::new();
        match MoFile::new_file(data) {
            Ok(mo_file) => for (orig, trans) in mo_file {
                translation.set(orig, trans);
            }
            Err(_) => error!("Invalid translation data")
        }
        translation
    }

    pub fn from_mo_file(path: &Path) -> Self {
        let mut translation = Translation::new();
        if let Ok(mut file) = File::open(&path) {
            let mut data = vec![];
            if file.read_to_end(&mut data).is_ok() {
                match MoFile::new_file(&data) {
                    Ok(mo_file) => for (orig, trans) in mo_file {
                        translation.set(orig.to_string(), trans.to_string());
                    }
                    Err(_) => error!("Invalid translation data")
                }
            }
        }
        translation
    }

    pub fn set<O: Into<CowStr>, T: Into<CowStr>>(&mut self, orig: O, trans: T) {
        let trans = trans.into();
        if !trans.is_empty() {
            self.0.insert(orig.into(), trans);
        }
    }

    pub fn get<'a, 'b: 'a>(&'b self, orig: &'a str) -> &'a str {
        self.0.get(orig).map(|s| s as &'a str).unwrap_or(orig)
    }
}

fn get_translation(locale: &str) -> Translation {
    if let Some(trans) = find_translation(locale) {
        return trans;
    }
    let country = locale.split('_').next().unwrap();
    if let Some(trans) = find_translation(country) {
        return trans;
    }
    Translation::new()
}

fn find_translation(name: &str) -> Option<Translation> {
    if EMBEDDED_TRANS.contains_key(name) {
        return Some(Translation::from_mo_data(EMBEDDED_TRANS[name]));
    }
    let path = PathBuf::from(format!("/usr/share/locale/{}/LC_MESSAGES/zvault.mo", name));
    if path.exists() {
        return Some(Translation::from_mo_file(&path));
    }
    let path = PathBuf::from(format!("lang/{}.mo", name));
    if path.exists() {
        return Some(Translation::from_mo_file(&path));
    }
    None
}

lazy_static! {
    pub static ref EMBEDDED_TRANS: HashMap<&'static str, &'static[u8]> = {
        HashMap::new()
        //map.insert("de", include_bytes!("../lang/de.mo") as &'static [u8]);
    };
    pub static ref TRANS: Translation = {
        let locale = Locale::current();
        let locale_str = locale.tags_for("").next().unwrap().as_ref().to_string();
        get_translation(&locale_str)
    };
}

#[macro_export] macro_rules! tr {
    ($fmt:tt) => (::translation::TRANS.get($fmt));
}

#[macro_export] macro_rules! tr_format {
    ($fmt:tt) => (tr!($fmt));
    ($fmt:tt, $($arg:tt)*) => (rt_format!(tr!($fmt), $($arg)*).expect("invalid format"));
}

#[macro_export] macro_rules! tr_println {
    ($fmt:tt) => (println!("{}", tr!($fmt)));
    ($fmt:tt, $($arg:tt)*) => (rt_println!(tr!($fmt), $($arg)*).expect("invalid format"));
}

#[macro_export] macro_rules! tr_trace {
    ($($arg:tt)*) => (debug!("{}", tr_format!($($arg)*)));
}

#[macro_export] macro_rules! tr_debug {
    ($($arg:tt)*) => (debug!("{}", tr_format!($($arg)*)));
}

#[macro_export] macro_rules! tr_info {
    ($($arg:tt)*) => (info!("{}", tr_format!($($arg)*)));
}

#[macro_export] macro_rules! tr_warn {
    ($($arg:tt)*) => (warn!("{}", tr_format!($($arg)*)));
}

#[macro_export] macro_rules! tr_error {
    ($($arg:tt)*) => (error!("{}", tr_format!($($arg)*)));
}

#[macro_export] macro_rules! tr_panic {
    ($($arg:tt)*) => (panic!("{}", tr_format!($($arg)*)));
}
