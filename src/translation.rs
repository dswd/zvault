use std::borrow::Cow;
use std::collections::HashMap;

type TransStr = Cow<'static, str>;

pub struct Translation(HashMap<TransStr, TransStr>);

impl Translation {
    pub fn new() -> Self {
        Translation(Default::default())
    }

    pub fn set<O: Into<TransStr>, T: Into<TransStr>>(&mut self, orig: O, trans: T) {
        self.0.insert(orig.into(), trans.into());
    }

    pub fn get<O: Into<TransStr>>(&self, orig: O) -> TransStr {
        let orig = orig.into();
        self.0.get(&orig).cloned().unwrap_or(orig)
    }
}

lazy_static! {
    static ref TRANS: Translation = {
        let mut trans = Translation::new();
        trans.set("Hello", "Hallo");
        trans
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

#[macro_export] macro_rules! tr_info {
    ($($arg:tt)*) => (info!("{}", tr_format!($($arg)*)));
}