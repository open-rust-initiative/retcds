use std::ops::Sub;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use crate::etcdserver::api::rafthttp::v2state::serialize_datetime;
use crate::etcdserver::api::rafthttp::v2state::deserialize_datetime;

const QUEUE_CAPACITY: isize = 200;

#[derive(Clone,Copy,Serialize,Deserialize,Debug)]
pub struct RequestState{
    #[serde(serialize_with = "serialize_datetime",deserialize_with = "deserialize_datetime")]
    sending_time : DateTime<Local>,
    size : isize
}

impl RequestState{
    pub fn new(st :DateTime<Local>,size : isize) -> Self{
        RequestState{
            sending_time: st,
            size: size
        }
    }

    pub fn size(&self) -> isize{
        self.size
    }

    pub fn sending_time(&self) -> DateTime<Local>{
        self.sending_time
    }
}



#[derive(Clone,Serialize,Deserialize,Debug)]
pub struct StateQueue{
    #[serde(serialize_with = "serialize_base_state_queue",deserialize_with = "deserialize_base_state_queue")]
    queue : Arc<RwLock<BaseStateQueue>>
}

fn serialize_base_state_queue<S>(
    data: &Arc<RwLock<BaseStateQueue>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
{
    data.read().unwrap().serialize(serializer)
}

fn deserialize_base_state_queue<'de, D>(
    deserializer: D,
) -> Result<Arc<RwLock<BaseStateQueue>>, D::Error>
    where
        D: serde::Deserializer<'de>,
{
    let base_state_queue = BaseStateQueue::deserialize(deserializer)?;
    Ok(Arc::new(RwLock::new(base_state_queue)))
}

impl StateQueue{
    pub fn new() ->Self{
        StateQueue{
            queue: Arc::new(RwLock::new(
                BaseStateQueue{
                    items: Vec::with_capacity(QUEUE_CAPACITY as usize),
                    size: 0,
                    front: 0,
                    back: -1,
                    total_req_size: 0,
                }
            ))
        }
    }

    pub fn rl(&self) -> RwLockReadGuard<'_, BaseStateQueue>{
        self.queue.read().unwrap()
    }

    pub fn wl(&self) -> RwLockWriteGuard<'_, BaseStateQueue>{
        self.queue.write().unwrap()
    }
}

#[derive(Clone,Serialize,Deserialize,Debug)]
pub struct BaseStateQueue{
    items : Vec<Option<RequestState>>,
    size : isize,
    front : isize,
    back : isize,
    total_req_size : isize,
}

impl BaseStateQueue{

    pub fn len(&self) -> isize{
        self.size
    }

    pub fn req_size(&self) -> isize{
        self.total_req_size
    }

    // FrontAndBack gets the front and back elements in the queue
    // We must grab front and back together with the protection of the lock
    pub fn front_and_back(&self) -> (Option<RequestState>, Option<RequestState>) {
        if self.size !=0{
            return (self.items[self.front as usize],self.items[self.back as usize])
        };
        return (None, None)
    }

    pub fn insert(&mut self, p:Option<RequestState>) {
        self.back = (self.back + 1) % QUEUE_CAPACITY;
        if self.size == QUEUE_CAPACITY {
            self.total_req_size -= self.items[self.front as usize].unwrap().size;
            self.front = (self.back + 1) % QUEUE_CAPACITY;
        } else {
            self.size += 1;
        }
        self.items[self.back as usize] = p;
        self.total_req_size += self.items[self.back as usize].unwrap().size;
    }

    // Rate function returns the package rate and byte rate
    pub fn rate(&mut self) -> (f64, f64){
        let (front, back) = self.front_and_back();
        if front.is_some() || back.is_some() {
          return (0.0, 0.0)
        };

        if Utc::now().signed_duration_since(back.unwrap().sending_time) > Duration::seconds(1){
          self.clear();
          return (0.0, 0.0)
        };

        let duration = back.unwrap().sending_time.sub(front.unwrap().sending_time);

        let pr = self.size as f64 / duration.num_seconds() as f64 * Duration::seconds(1).num_milliseconds() as f64;

        let br = self.total_req_size as f64 / duration.num_seconds() as f64 * Duration::seconds(1).num_milliseconds() as f64;

        return (pr,br)
    }

    pub fn clear(&mut self){
        self.back = -1;
        self.front =0;
        self.size =0;
        self.total_req_size = 0;
    }

}