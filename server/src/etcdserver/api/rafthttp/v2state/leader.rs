use std::sync::Mutex;
use chrono::Duration;

struct FollowerStats{
    base_follower_stats : Mutex<BaseFollowerStats>,
}

impl FollowerStats{
    fn succ(&self,d : Duration){
        let _lock = self.base_follower_stats.lock().unwrap();

    }
}

struct BaseFollowerStats{
    latency : LatencyStats,
    counts : CountsStats,
}

struct CountsStats{
    fail : u64,
    success : u64,
}

struct LatencyStats{
    current : f64,
    average : f64,
    average_square : f64,
    standard_deviation : f64,
    minimum : f64,
    maximum : f64,
}