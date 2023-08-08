use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use chrono::Duration;
use slog::error;
use std::borrow::Borrow;
use serde::{Deserialize, Serialize};
use crate::etcdserver::api::rafthttp::v2state::default_logger;

#[derive(Serialize,Clone,Deserialize,Debug)]
pub struct LeaderStats{
    leader: String,
    followers : HashMap<String, FollowerStats>
}

impl LeaderStats {
    fn JSON(&mut self) -> Vec<u8> {
        // let base_leader_stats = self.base_leader_stats.clone();
        // let base_leader_stats_guard = base_leader_stats.lock().unwrap();
        let result = serde_json::to_vec(&self);
        if let Err(err) = result {
            error!(default_logger(), "failed to serialize leader stats"; "error" => %err);
            return vec![];
        }
        result.unwrap()
    }
    fn follower(&mut self, name: String) -> FollowerStats {
        // let mut _lock = match self.base_leader_stats.lock() {
        //     Ok(guard) => guard,
        //     Err(poisoned) => poisoned.into_inner(),
        // };
        let mut option = self.followers.get(&*name);
        let fs = option.clone();

        match option.clone() {
            Some(fs) => fs,
            None => {
                let fs = FollowerStats::default();
                self.followers.insert(name.clone(), fs);
                return self.followers.get(&*name).unwrap().clone();
            }
        };
        fs.cloned().unwrap()
    }
}



#[derive(Clone,Serialize,Deserialize,Debug)]
pub struct FollowerStats{
    latency : LatencyStats,
    counts : CountsStats,
}

impl FollowerStats{
    pub fn default() -> Self{
        FollowerStats{
            counts: CountsStats {
                success: 0,
                fail: 0,
            },
            latency: LatencyStats {
                current: 0.0,
                average: 0.0,
                average_square: 0.0,
                standard_deviation: 0.0,
                minimum: 0.0,
                maximum: 0.0,
            },
        }
    }

    pub fn get_latency(&self) -> LatencyStats{
        self.latency.clone()
    }

    pub fn get_counts(&self) -> CountsStats{
        self.counts.clone()
    }

    pub fn succ(&mut self, d : Duration){
        let total = self.counts.success as f64 * self.latency.average as f64;
        let total_square = self.counts.success as f64 * self.latency.average_square as f64;
        self.counts.success += 1;

        self.latency.current = d.num_milliseconds() as f64/(1000000.0);

        if self.latency.current >  self.latency.maximum {
            self.latency.maximum = self.latency.current;
        };

        if self.latency.current < self.latency.minimum {
            self.latency.minimum = self.latency.current;
        };

        self.latency.average = (total + self.latency.current) / self.counts.success as f64;
        self.latency.average_square = (total_square + self.latency.current * self.latency.current) / self.counts.success as f64;
        self.latency.standard_deviation = (self.latency.average_square - self.latency.average * self.latency.average).sqrt();
    }

    pub fn fail(&mut self){
        self.counts.fail += 1;
    }
}

#[derive(Clone,Serialize,Deserialize,Debug)]
pub struct CountsStats{
    fail : u64,
    success : u64,
}

impl CountsStats{
    pub fn get_fail(&self) -> u64{
        self.fail
    }

    pub fn get_success(&self) -> u64{
        self.success
    }
}

#[derive(Clone,Serialize,Deserialize,Debug)]
pub struct LatencyStats{
    current : f64,
    average : f64,
    average_square : f64,
    standard_deviation : f64,
    minimum : f64,
    maximum : f64,
}