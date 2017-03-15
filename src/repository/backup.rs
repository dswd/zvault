use super::{Repository, Chunk};

use rmp_serde;
use serde::{Deserialize, Serialize};

use std::fs::{self, File};
use std::path::Path;


#[derive(Default, Debug)]
pub struct Backup {
    pub root: Vec<Chunk>,
    pub total_data_size: u64,
    pub changed_data_size: u64,
    pub new_data_size: u64,
    pub encoded_data_size: u64,
    pub new_bundle_count: usize,
    pub chunk_count: usize,
    pub avg_chunk_size: f32,
    pub date: i64,
    pub duration: f32,
    pub file_count: usize,
    pub dir_count: usize
}
serde_impl!(Backup(u8) {
    root: Vec<Chunk> => 0,
    total_data_size: u64 => 1,
    changed_data_size: u64 => 2,
    new_data_size: u64 => 3,
    encoded_data_size: u64 => 4,
    new_bundle_count: usize => 5,
    chunk_count: usize => 6,
    avg_chunk_size: f32 => 7,
    date: i64 => 8,
    duration: f32 => 9,
    file_count: usize => 10,
    dir_count: usize => 11
});


impl Repository {
    pub fn list_backups(&self) -> Result<Vec<String>, &'static str> {
        let mut backups = Vec::new();
        let mut paths = Vec::new();
        let base_path = self.path.join("backups");
        paths.push(base_path.clone());
        while let Some(path) = paths.pop() {
            for entry in try!(fs::read_dir(path).map_err(|_| "Failed to list files")) {
                let entry = try!(entry.map_err(|_| "Failed to list files"));
                let path = entry.path();
                if path.is_dir() {
                    paths.push(path);
                } else {
                    let relpath = try!(path.strip_prefix(&base_path).map_err(|_| "Failed to obtain relative path"));
                    backups.push(relpath.to_string_lossy().to_string());
                }
            }
        }
        Ok(backups)
    }

    pub fn get_backup(&self, name: &str) -> Result<Backup, &'static str> {
        let file = try!(File::open(self.path.join("backups").join(name)).map_err(|_| "Failed to load backup"));
        let mut reader = rmp_serde::Deserializer::new(file);
        Backup::deserialize(&mut reader).map_err(|_| "Failed to read backup data")
    }

    pub fn save_backup(&mut self, backup: &Backup, name: &str) -> Result<(), &'static str> {
        let mut file = try!(File::create(self.path.join("backups").join(name)).map_err(|_| "Failed to save backup"));
        let mut writer = rmp_serde::Serializer::new(&mut file);
        backup.serialize(&mut writer).map_err(|_| "Failed to write backup data")
    }

    pub fn restore_backup<P: AsRef<Path>>(&mut self, backup: &Backup, path: P) -> Result<(), &'static str> {
        let inode = try!(self.get_inode(&backup.root));
        try!(self.save_inode_at(&inode, path));
        Ok(())
    }
}
