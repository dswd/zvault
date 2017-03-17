extern crate serde;
extern crate rmp_serde;
#[macro_use] extern crate serde_utils;
extern crate squash_sys as squash;
extern crate mmap;
extern crate blake2_rfc as blake2;
extern crate murmurhash3;
extern crate serde_yaml;
#[macro_use] extern crate quick_error;
extern crate docopt;
extern crate rustc_serialize;
extern crate chrono;
#[macro_use] extern crate clap;

pub mod util;
pub mod bundle;
pub mod index;
mod chunker;
mod repository;
mod cli;

// TODO: Seperate remote folder
// TODO: Copy backup files to remote folder
// TODO: Keep meta bundles also locally
// TODO: Remove backups (based on age like attic)
// TODO: Backup files tree structure
// TODO: Recompress & combine bundles
// TODO: Check backup integrity
// TODO: Encryption
// TODO: list --tree
// TODO: Partial backups
// TODO: Load and compare remote bundles to bundle map
// TODO: Nice errors / checks for CLI
// TODO: Import remote backup
// TODO: Continue on errors

fn main() {
    cli::run();
}
