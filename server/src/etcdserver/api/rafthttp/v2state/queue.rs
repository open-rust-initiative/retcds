use std::ops::Sub;
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use actix_web::body::MessageBody;
use actix_web::cookie::time::Time;
use chrono::{DateTime, Duration, Local, Utc};

const QUEUE_CAPACITY: isize = 200;

#[derive(Clone,Copy)]
pub struct RequestState{
    sending_time : DateTime<Local>,
    size : isize
}

pub struct StateQueue{
    pub(crate) queue : Arc<RwLock<BaseStateQueue>>
}

impl StateQueue{
    pub fn new(&self) ->Self{
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

pub struct BaseStateQueue{
    items : Vec<Option<RequestState>>,
    size : isize,
    front : isize,
    back : isize,
    total_req_size : isize,
}

impl BaseStateQueue{


    fn len(&self) -> isize{
        self.size
    }

    fn req_size(&self) -> isize{
        self.total_req_size
    }

    // FrontAndBack gets the front and back elements in the queue
    // We must grab front and back together with the protection of the lock
    fn front_and_back(&self) -> (Option<RequestState>, Option<RequestState>) {
        if self.size !=0{
            return (self.items[self.front as usize],self.items[self.back as usize])
        };
        return (None, None)
    }

    fn insert(&mut self, p:Option<RequestState>) {
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
    fn rate(&mut self) -> (f64, f64){
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

    fn clear(&mut self){
        self.back = -1;
        self.front =0;
        self.size =0;
        self.total_req_size = 0;
    }

}