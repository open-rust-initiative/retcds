use std::ops::Deref;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Local};
use openssl_sys::system;
use serde::{Serialize,Deserialize};
use slog::info;
use raft::StateRole;
use crate::etcdserver::api::rafthttp::v2state::queue::{RequestState, StateQueue};
use crate::etcdserver::api::rafthttp::v2state::default_logger;
use crate::etcdserver::api::rafthttp::v2state::serialize_datetime;
use crate::etcdserver::api::rafthttp::v2state::deserialize_datetime;


// #[derive(Serialize,Deserialize,Clone,Debug)]
// pub struct ServerState{
//     #[serde(serialize_with = "serialize_base_server_state",deserialize_with = "deserialize_base_server_state")]
//     server_state : Arc<Mutex<BaseServerState>>,
// }
//
// fn serialize_base_server_state<S>(
//     data: &Arc<Mutex<BaseServerState>>,
//     serializer: S,
// ) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
// {
//     data.lock().unwrap().serialize(serializer)
// }
//
// fn deserialize_base_server_state<'de, D>(
//     deserializer: D,
// ) -> Result<Arc<Mutex<BaseServerState>>, D::Error>
//     where
//         D: serde::Deserializer<'de>,
// {
//     let base_server_state = BaseServerState::deserialize(deserializer)?;
//     Ok(Arc::new(Mutex::new(base_server_state)))
// }

fn new_server_state(name: String, id:String) -> ServerState {
    let server_state =
        ServerState {
            name,
            id,
            state: StateRole::Follower,
            start_time: Local::now(),
            leader_info: leader_info {
                name: "".to_string(),
                up_time: "".to_string(),
                start_time: Local::now(),
            },
            recv_append_request_cnt: 0,
            recving_pkg_rate: 0.0,
            recving_bandwidth_rate: 0.0,
            send_append_request_cnt: 0,
            sending_pkg_rate: 0.0,
            sending_bandwidth_rate: 0.0,
            send_rate_queue: StateQueue::new(),
            recv_rate_queue: StateQueue::new(),
        };
    server_state
}

#[derive(Clone,Serialize,Deserialize,Debug)]
pub struct ServerState{

    name: String,
    id: String,
    state : StateRole,

    #[serde(serialize_with = "serialize_datetime",deserialize_with = "deserialize_datetime")]
    start_time : DateTime<Local>,

    leader_info : leader_info,

    recv_append_request_cnt : u64,
    recving_pkg_rate : f64,
    recving_bandwidth_rate : f64,

    send_append_request_cnt : u64,
    sending_pkg_rate : f64,
    sending_bandwidth_rate : f64,

    send_rate_queue : StateQueue,
    recv_rate_queue : StateQueue
}

impl ServerState{
    fn JSON(&mut self) -> Vec<u8> {
        let (sending_pkg_rate, sending_bandwidth_rate) = self.send_rate_queue.wl().rate();
        let (recving_pkg_rate, recving_bandwidth_rate) = self.recv_rate_queue.wl().rate();

        self.sending_pkg_rate = sending_pkg_rate;
        self.sending_bandwidth_rate = sending_bandwidth_rate;
        self.recving_pkg_rate = recving_pkg_rate;
        self.recving_bandwidth_rate = recving_bandwidth_rate;

        let result = serde_json::to_vec(self);
        if let Err(err) = result {
            info!(default_logger(), "failed to serialize leader stats"; "error" => %err);
            return vec![];
        };
        return result.unwrap();
    }

    fn recv_append_req(&mut self, leader:String, req_size: isize){
        let now = Local::now();
        self.state = StateRole::Follower;

        if leader != self.leader_info.name {
            self.leader_info.name = leader;
            self.leader_info.start_time = now;
        }

        let state = RequestState::new(now, req_size);
        self.recv_rate_queue.wl().insert(Option::from(state));
        self.recv_append_request_cnt += 1
    }

    fn send_append_req(&mut self, req_size: isize){
        self.become_leader();
        let state = RequestState::new(Local::now(), req_size);

        self.send_rate_queue.wl().insert(Option::from(state));
        self.send_append_request_cnt += 1
    }

    pub fn become_leader(&mut self){
        if self.state != StateRole::Leader {
            self.state = StateRole::Leader;
            self.leader_info.name = self.id.clone();
            self.leader_info.start_time = Local::now();
        }
    }

}

#[derive(Clone,Serialize,Deserialize,Debug)]
pub struct  leader_info{
    name : String,
    up_time: String,
    #[serde(serialize_with = "serialize_datetime",deserialize_with = "deserialize_datetime")]
    start_time : DateTime<Local>,
}