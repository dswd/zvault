use pbr;
use std::io::Stdout;
use std::time::Duration;

pub fn to_file_size(size: u64) -> String {
    let mut size = size as f32;
    if size >= 512.0 {
        size /= 1024.0;
    } else {
        return format!("{:.0} Byte", size);
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
        ProgressIter {
            inner: inner,
            bar: bar,
            msg: msg
        }
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
                let msg = self.msg.clone() + tr!("done.");
                self.bar.finish_print(&msg);
                None
            }
            Some(item) => {
                self.bar.inc();
                Some(item)
            }
        }
    }
}


mod tests {

    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_to_file_size() {
        assert_eq!("0 Byte", to_file_size(0));
        assert_eq!("1 Byte", to_file_size(1));
        assert_eq!("15 Byte", to_file_size(15));
        assert_eq!("456 Byte", to_file_size(456));
        assert_eq!("0.7 KiB", to_file_size(670));
        assert_eq!("237.0 KiB", to_file_size(242670));
        assert_eq!("442.5 KiB", to_file_size(453170));
        assert_eq!("0.7 MiB", to_file_size(753170));
        assert_eq!("12.2 MiB", to_file_size(12753170));
        assert_eq!("222.0 MiB", to_file_size(232753170));
        assert_eq!("5.1 GiB", to_file_size(5435353170));
        assert_eq!("291.1 GiB", to_file_size(312534553170));
        assert_eq!("3.9 TiB", to_file_size(4312534553170));
    }

    #[test]
    fn test_to_speed() {
        assert_eq!("0 Byte/s", to_speed(0, 1.0));
        assert_eq!("100 Byte/s", to_speed(100, 1.0));
        assert_eq!("1.0 KiB/s", to_speed(100, 0.1));
        assert_eq!("10 Byte/s", to_speed(100, 10.0));
        assert_eq!("237.0 KiB/s", to_speed(242670, 1.0));
        assert_eq!("0.7 MiB/s", to_speed(753170, 1.0));
        assert_eq!("222.0 MiB/s", to_speed(232753170, 1.0));
        assert_eq!("291.1 GiB/s", to_speed(312534553170, 1.0));
        assert_eq!("3.9 TiB/s", to_speed(4312534553170, 1.0));
    }

    #[test]
    fn test_to_duration() {
        assert_eq!("0:00:00.0", to_duration(0.0));
        assert_eq!("0:00:00.1", to_duration(0.1));
        assert_eq!("0:00:01.0", to_duration(1.0));
        assert_eq!("0:01:00.0", to_duration(60.0));
        assert_eq!("1:00:00.0", to_duration(3600.0));
        assert_eq!("2:02:02.2", to_duration(7322.2));
    }


}
