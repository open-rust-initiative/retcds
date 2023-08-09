use std::sync::Arc;
use std::time::SystemTime;
use slog::{debug, info, warn};
use tokio::sync::Mutex;
use crate::etcdserver::api::rafthttp::types;
use crate::etcdserver::api::rafthttp::default_logger;
pub struct FailureType {
    source: String,
    action: String,
}

impl FailureType {
    pub fn new_failure_type(source: String, action: String) -> FailureType {
        FailureType {
            source,
            action,
        }
    }
}

// #[derive(Clone,Debug)]
// pub struct PeerStatus{
//     base_peer_status : Arc<Mutex<BasePeerStatus>>
// }

// impl PeerStatus{
//     pub fn new_peer_status(local: types::id::ID, id: types::id::ID) -> PeerStatus {
//         PeerStatus{
//             base_peer_status: Arc::new(Mutex::new(BasePeerStatus::new(local,id)))
//         }
//     }
//     pub fn get_base_peer_status(&self) -> Arc<Mutex<BasePeerStatus>>{
//         self.base_peer_status.clone()
//     }
// }

#[derive(Debug)]
pub struct PeerStatus {
    lg: slog::Logger,
    local: types::id::ID,
    id: types::id::ID,
    active: bool,
    since: Option<SystemTime>,
}

impl PeerStatus{
    pub fn new(local: types::id::ID, id: types::id::ID) -> PeerStatus {
        PeerStatus {
            lg: default_logger(),
            local,
            id,
            active: false,
            since: None,
        }
    }

    pub fn activate(&mut self){
        if !self.active{
            info!(self.lg,"peer became active peer-id=>{}",self.id.to_string());
            self.active = true;
            self.since = Some(SystemTime::now());
        }
    }

    pub fn deactivate(&mut self,failure:FailureType,reason:String){
        let msg = format!("failed to {} {} on {} {}",failure.action,self.id.to_string(),failure.source,reason);
        if self.active{
            warn!(self.lg,"peer became inactive(message send to peer failed) peer-id=>{} error =>{}",self.id.to_string(),msg.clone());
            self.active = false;
            self.since = None;
            return;
        }
        debug!(self.lg,"peer deactivated again peer-id=>{} error=>{}",self.id.to_string(),msg.clone());
    }

    pub fn is_active(&self) -> bool{
        self.active
    }

    pub fn active_since(&self) -> Option<SystemTime>{
        self.since
    }
}
