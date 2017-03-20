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


pub mod util;
pub mod bundle;
pub mod index;
mod chunker;
mod repository;
mod cli;

// TODO: Seperate remote folder
// TODO: - Copy/move backup files to remote folder
// TODO: - Keep meta bundles also locally
// TODO: - Load and compare remote bundles to bundle map
// TODO: - Write backup files there as well
// TODO: Remove backup subtrees
// TODO: Recompress & combine bundles
// TODO: Encrypt backup files too
// TODO: list --tree
// TODO: Partial backups
// TODO: Import repository from remote folder
// TODO: Continue on errors

fn main() {
    cli::run();
}
