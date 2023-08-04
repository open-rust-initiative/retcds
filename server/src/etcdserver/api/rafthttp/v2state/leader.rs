use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use actix::fut::{err, result};
use actix_web::web::Json;
use bincode::Options;
use chrono::Duration;
use openssl_sys::open;
use slog::error;
use std::borrow::Borrow;
use std::ops::Deref;
use serde::{Deserialize, Serialize};
use crate::etcdserver::api::rafthttp::v2state::default_logger;
use crate::etcdserver::api::rafthttp::v2state::serialize_datetime;
use crate::etcdserver::api::rafthttp::v2state::deserialize_datetime;

#[derive(Serialize,Clone,Deserialize)]
pub struct LeaderStats{
    #[serde(skip,default = "default_logger")]
    logger : slog::Logger,
    #[serde(serialize_with = "serialize_base_leader_stats",deserialize_with = "deserialize_base_leader_stats")]
    base_leader_stats : Arc<Mutex<BaseLeaderStats>>,
}



fn serialize_base_leader_stats<S>(
    data: &Arc<Mutex<BaseLeaderStats>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
{
    data.lock().unwrap().serialize(serializer)
}

fn deserialize_base_leader_stats<'de, D>(
    deserializer: D,
) -> Result<Arc<Mutex<BaseLeaderStats>>, D::Error>
    where
        D: serde::Deserializer<'de>,
{
    let base_leader_stats = BaseLeaderStats::deserialize(deserializer)?;
    Ok(Arc::new(Mutex::new(base_leader_stats)))
}


impl LeaderStats {
    fn JSON(&mut self) -> Vec<u8> {
        let base_leader_stats = self.base_leader_stats.clone();
        let base_leader_stats_guard = base_leader_stats.lock().unwrap();
        let result = serde_json::to_vec(&*base_leader_stats_guard);
        if let Err(err) = result {
            error!(self.logger, "failed to serialize leader stats"; "error" => %err);
            return vec![];
        }
        result.unwrap()
    }
    fn follower(&self, name: String) -> FollowerStats {
        let mut _lock = match self.base_leader_stats.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let mut option = _lock.followers.get(&*name);
        let fs = option.clone();

        match option.clone() {
            Some(fs) => fs,
            None => {
                let fs = FollowerStats::default();
                _lock.followers.insert(name.clone(), fs);
                return _lock.followers.get(&*name).unwrap().clone();
            }
        };
        fs.cloned().unwrap()
    }
}

#[derive(Serialize,Clone,Deserialize)]
struct BaseLeaderStats{
    leader: String,
    followers : HashMap<String, FollowerStats>
}

#[derive(Serialize,Deserialize)]
pub struct FollowerStats{
    base_follower_stats : Mutex<BaseFollowerStats>,
}

impl Clone for FollowerStats {
    fn clone(&self) -> Self {
        Self{
            base_follower_stats: Mutex::new(self.base_follower_stats.lock().unwrap().clone())
        }
    }
}

// Succ updates the FollowerStats with a successful send
impl FollowerStats{

    fn default() -> Self{
        FollowerStats{
            base_follower_stats: Mutex::new(BaseFollowerStats{
                counts: CountsStats{
                    success: 0,
                    fail: 0,
                },
                latency: LatencyStats{
                    current: 0.0,
                    average: 0.0,
                    average_square: 0.0,
                    standard_deviation: 0.0,
                    minimum: 0.0,
                    maximum: 0.0,
                },
            })
        }
    }

    fn succ(&self,d : Duration){
        let mut _lock = match self.base_follower_stats.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let total = _lock.counts.success as f64 * _lock.latency.average as f64;
        let total_square = _lock.counts.success as f64 * _lock.latency.average_square as f64;
        _lock.counts.success += 1;

        _lock.latency.current = d.num_milliseconds() as f64/(1000000.0);

        if _lock.latency.current >  _lock.latency.maximum {
            _lock.latency.maximum = _lock.latency.current;
        };

        if _lock.latency.current < _lock.latency.minimum {
            _lock.latency.minimum = _lock.latency.current;
        };

        _lock.latency.average = (total + _lock.latency.current) / _lock.counts.success as f64;
        _lock.latency.average_square = (total_square + _lock.latency.current * _lock.latency.current) / _lock.counts.success as f64;
        _lock.latency.standard_deviation = (_lock.latency.average_square - _lock.latency.average * _lock.latency.average).sqrt();
        std::mem::drop(_lock);
    }

    fn fail(&self){
        let mut _lock = match self.base_follower_stats.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        _lock.counts.fail += 1;
        std::mem::drop(_lock);
    }
}

#[derive(Clone,Serialize,Deserialize)]
struct BaseFollowerStats{
    latency : LatencyStats,
    counts : CountsStats,
}

#[derive(Clone,Serialize,Deserialize)]
struct CountsStats{
    fail : u64,
    success : u64,
}

#[derive(Clone,Serialize,Deserialize)]
struct LatencyStats{
    current : f64,
    average : f64,
    average_square : f64,
    standard_deviation : f64,
    minimum : f64,
    maximum : f64,
}