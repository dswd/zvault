mod config;
mod bundle_map;
mod layout;
mod error;
mod inner;
pub mod bundledb;
pub mod index;
pub mod chunking;


pub use self::error::RepositoryError;
pub use self::config::Config;
pub use self::inner::IntegrityError;
pub use self::inner::{RepositoryInfo, BundleAnalysis, RepositoryStatistics};
pub use self::layout::{RepositoryLayout, ChunkRepositoryLayout};
pub use self::inner::{Location, RepositoryInner};

pub use self::inner::api::*;


const REPOSITORY_README: &[u8] = include_bytes!("../../docs/repository_readme.md");

const INDEX_MAGIC: [u8; 7] = *b"zvault\x02";
const INDEX_VERSION: u8 = 1;