mod config;
mod bundle_map;
mod integrity;
mod basic_io;
mod info;

use std::mem;
use std::cmp::max;
use std::path::{PathBuf, Path};
use std::fs;

use super::index::Index;
use super::bundle::{BundleDb, BundleWriter};
use super::chunker::Chunker;

pub use self::config::Config;
use self::bundle_map::BundleMap;


#[derive(Eq, Debug, PartialEq, Clone, Copy)]
pub enum Mode {
    Content, Meta
}

pub struct Repository {
    path: PathBuf,
    config: Config,
    index: Index,
    bundle_map: BundleMap,
    next_content_bundle: u32,
    next_meta_bundle: u32,
    bundles: BundleDb,
    content_bundle: Option<BundleWriter>,
    meta_bundle: Option<BundleWriter>,
    chunker: Chunker
}


impl Repository {
    pub fn create<P: AsRef<Path>>(path: P, config: Config) -> Result<Self, &'static str> {
        let path = path.as_ref().to_owned();
        try!(fs::create_dir(&path).map_err(|_| "Failed to create repository directory"));
        let bundles = try!(BundleDb::create(
            path.join("bundles"),
            config.compression.clone(),
            None, //FIXME: store encryption in config
            config.checksum
        ).map_err(|_| "Failed to create bundle db"));
        let index = try!(Index::create(&path.join("index")).map_err(|_| "Failed to create index"));
        try!(config.save(path.join("config.yaml")).map_err(|_| "Failed to save config"));
        let bundle_map = BundleMap::create();
        try!(bundle_map.save(path.join("bundles.map")).map_err(|_| "Failed to save bundle map"));
        Ok(Repository{
            path: path,
            chunker: config.chunker.create(),
            config: config,
            index: index,
            bundle_map: bundle_map,
            next_content_bundle: 1,
            next_meta_bundle: 0,
            bundles: bundles,
            content_bundle: None,
            meta_bundle: None,
        })
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, &'static str> {
        let path = path.as_ref().to_owned();
        let config = try!(Config::load(path.join("config.yaml")).map_err(|_| "Failed to load config"));
        let bundles = try!(BundleDb::open(
            path.join("bundles"),
            config.compression.clone(),
            None, //FIXME: load encryption from config
            config.checksum
        ).map_err(|_| "Failed to open bundle db"));
        let index = try!(Index::open(&path.join("index")).map_err(|_| "Failed to open index"));
        let bundle_map = try!(BundleMap::load(path.join("bundles.map")).map_err(|_| "Failed to load bundle map"));
        let mut repo = Repository {
            path: path,
            chunker: config.chunker.create(),
            config: config,
            index: index,
            bundle_map: bundle_map,
            next_content_bundle: 0,
            next_meta_bundle: 0,
            bundles: bundles,
            content_bundle: None,
            meta_bundle: None,
        };
        repo.next_meta_bundle = repo.next_free_bundle_id();
        repo.next_content_bundle = repo.next_free_bundle_id();
        Ok(repo)
    }

    #[inline]
    fn save_bundle_map(&self) -> Result<(), &'static str> {
        self.bundle_map.save(self.path.join("bundles.map"))
    }

    #[inline]
    fn next_free_bundle_id(&self) -> u32 {
        let mut id = max(self.next_content_bundle, self.next_meta_bundle) + 1;
        while self.bundle_map.get(id).is_some() {
            id += 1;
        }
        id
    }

    pub fn flush(&mut self) -> Result<(), &'static str> {
        if self.content_bundle.is_some() {
            let mut finished = None;
            mem::swap(&mut self.content_bundle, &mut finished);
            {
                let bundle = try!(self.bundles.add_bundle(finished.unwrap()).map_err(|_| "Failed to write finished bundle"));
                self.bundle_map.set(self.next_content_bundle, bundle);
            }
            self.next_content_bundle = self.next_free_bundle_id()
        }
        if self.meta_bundle.is_some() {
            let mut finished = None;
            mem::swap(&mut self.meta_bundle, &mut finished);
            {
                let bundle = try!(self.bundles.add_bundle(finished.unwrap()).map_err(|_| "Failed to write finished bundle"));
                self.bundle_map.set(self.next_meta_bundle, bundle);
            }
            self.next_meta_bundle = self.next_free_bundle_id()
        }
        try!(self.save_bundle_map().map_err(|_| "Failed to save bundle map"));
        Ok(())
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        self.flush().expect("Failed to write last bundles")
    }
}
