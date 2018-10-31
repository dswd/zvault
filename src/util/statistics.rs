

#[derive(Debug, Default)]
pub struct ValueStats {
    pub min: f32,
    pub max: f32,
    pub avg: f32,
    pub stddev: f32,
    pub count: usize,
    pub count_xs: usize,
    pub count_s: usize,
    pub count_m: usize,
    pub count_l: usize,
    pub count_xl: usize,
}

impl ValueStats {
    pub fn from_sequence<T: Iterator<Item=f32>, F: Fn() -> T>(iter: F) -> ValueStats {
        let mut stats = ValueStats::default();
        stats.min = ::std::f32::INFINITY;
        let mut sum = 0.0f64;
        for val in iter() {
            if stats.min > val {
                stats.min = val;
            }
            if stats.max < val {
                stats.max = val;
            }
            sum += f64::from(val);
            stats.count += 1;
        }
        stats.avg = (sum as f32) / (stats.count as f32);
        if stats.count < 2 {
            stats.count_m = stats.count;
            return stats;
        }
        sum = 0.0;
        for val in iter() {
            sum += f64::from(val - stats.avg) * f64::from(val - stats.avg);
        }
        stats.stddev = ((sum as f32)/(stats.count as f32-1.0)).sqrt();
        for val in iter() {
            if val < stats.avg - 2.0 * stats.stddev {
                stats.count_xs += 1;
            } else if val < stats.avg - stats.stddev {
                stats.count_s += 1;
            } else if val < stats.avg + stats.stddev {
                stats.count_m += 1;
            } else if val < stats.avg + 2.0 * stats.stddev {
                stats.count_l += 1;
            } else {
                stats.count_xl += 1;
            }
        }
        stats
    }
}