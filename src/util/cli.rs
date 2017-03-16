use ::repository::{Inode, FileType};

pub fn split_repo_path(repo_path: &str) -> (&str, Option<&str>, Option<&str>) {
    let mut parts = repo_path.splitn(3, "::");
    let repo = parts.next().unwrap();
    let backup = parts.next();
    let inode = parts.next();
    (repo, backup, inode)
}

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

pub fn to_duration(dur: f32) -> String {
    let secs = dur.floor() as u64;
    let subsecs = dur - dur.floor();
    let hours = secs / 3600;
    let mins = (secs / 60) % 60;
    let secs = (secs % 60) as f32 + subsecs;
    format!("{}:{:02}:{:04.1}", hours, mins, secs)
}

pub fn format_inode_one_line(inode: &Inode) -> String {
    match inode.file_type {
        FileType::Directory => format!("{:25}\t{} entries", format!("{}/", inode.name), inode.children.as_ref().unwrap().len()),
        FileType::File => format!("{:25}\t{}", inode.name, to_file_size(inode.size)),
        FileType::Symlink => format!("{:25}\t -> {}", inode.name, inode.symlink_target.as_ref().unwrap()),
    }
}
