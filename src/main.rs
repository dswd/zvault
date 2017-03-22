#![recursion_limit="128"]
extern crate serde;
extern crate rmp_serde;
#[macro_use] extern crate serde_utils;
extern crate squash_sys as squash;
extern crate mmap;
extern crate blake2_rfc as blake2;
extern crate murmurhash3;
extern crate serde_yaml;
#[macro_use] extern crate quick_error;
extern crate rustc_serialize;
extern crate chrono;
#[macro_use] extern crate clap;
#[macro_use] extern crate log;
extern crate byteorder;
extern crate sodiumoxide;
extern crate ansi_term;
extern crate libc;

pub mod util;
pub mod bundledb;
pub mod index;
mod chunker;
mod repository;
mod cli;
mod prelude;

// TODO: React on changes in remote bundles
// TODO: Lock during backup and vacuum
// TODO: Remove backup subtrees
// TODO: Recompress & combine bundles
// TODO: list --tree
// TODO: Import repository from remote folder
// TODO: Continue on errors (return summary as error)
// TODO: More detailed errors with nicer text
// TODO: Allow to use tar files for backup and restore (--tar, http://alexcrichton.com/tar-rs/tar/index.html)

fn main() {
    cli::run();
}
