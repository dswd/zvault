#![recursion_limit="128"]
#![allow(unknown_lints, float_cmp)]
#![cfg_attr(feature = "bench", feature(test))]
#[cfg(feature = "bench")]
extern crate test;
extern crate serde;
extern crate serde_bytes;
extern crate rmp_serde;
#[macro_use]
extern crate serde_utils;
extern crate squash_sys as squash;
extern crate blake2_rfc as blake2;
extern crate murmurhash3;
extern crate serde_yaml;
#[macro_use]
extern crate quick_error;
extern crate chrono;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate byteorder;
extern crate sodiumoxide;
extern crate ansi_term;
extern crate filetime;
extern crate regex;
#[macro_use]
extern crate lazy_static;
extern crate fuse;
extern crate rand;
extern crate time;
extern crate xattr;
extern crate crossbeam;
extern crate pbr;
extern crate users;
extern crate libc;
extern crate tar;
#[macro_use]
extern crate runtime_fmt;
extern crate locale_config;
extern crate mmap;

#[macro_use] mod translation;
pub mod util;
mod repository;
mod cli;
mod prelude;
mod backups;

use std::process::exit;

fn main() {
    match cli::run() {
        Ok(()) => exit(0),
        Err(code) => exit(code.code()),
    }
}
