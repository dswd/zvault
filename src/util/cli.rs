use pbr;
use std::io::Stdout;
use std::time::Duration;

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

pub fn to_speed(size: u64, dur: f32) -> String {
    let speed = (size as f32 / dur) as u64;
    to_file_size(speed) + "/s"
}

pub fn to_duration(dur: f32) -> String {
    let secs = dur.floor() as u64;
    let subsecs = dur - dur.floor();
    let hours = secs / 3600;
    let mins = (secs / 60) % 60;
    let secs = (secs % 60) as f32 + subsecs;
    format!("{}:{:02}:{:04.1}", hours, mins, secs)
}


pub struct ProgressIter<T> {
    inner: T,
    msg: String,
    bar: pbr::ProgressBar<Stdout>
}

impl<T> ProgressIter<T> {
    #[allow(blacklisted_name)]
    pub fn new(msg: &str, max: usize, inner: T) -> Self {
        let mut bar = pbr::ProgressBar::new(max as u64);
        let msg = format!("{}: ", msg);
        bar.message(&msg);
        bar.set_max_refresh_rate(Some(Duration::from_millis(100)));
        ProgressIter { inner: inner, bar: bar, msg: msg }
    }
}

impl<T: Iterator> Iterator for ProgressIter<T> {
    type Item = T::Item;

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next() {
            None => {
                let msg = self.msg.clone() + "done.";
                self.bar.finish_print(&msg);
                None
            },
            Some(item) => {
                self.bar.inc();
                Some(item)
            }
        }
    }
}
