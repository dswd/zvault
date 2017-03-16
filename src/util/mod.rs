mod checksum;
mod compression;
mod encryption;
mod hash;
mod lru_cache;
pub mod msgpack;

pub use self::checksum::*;
pub use self::compression::*;
pub use self::encryption::*;
pub use self::hash::*;
pub use self::lru_cache::*;


pub fn to_file_size(size: u64) -> String {
    let mut size = size as f32;
    if size >= 512.0 {
        size /= 1024.0;
    } else {
        return format!("{:.0} Bytes", size);
    }
    if size >= 512.0 {
        size /= 1024.0;
    } else {
        return format!("{:.1} KiB", size);
    }
    if size >= 512.0 {
        size /= 1024.0;
    } else {
        return format!("{:.1} MiB", size);
    }
    if size >= 512.0 {
        size /= 1024.0;
    } else {
        return format!("{:.1} GiB", size);
    }
    format!("{:.1} TiB", size)
}
