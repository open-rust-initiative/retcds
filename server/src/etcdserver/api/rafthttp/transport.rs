use std::env::Args;
use actix_web::Handler;
use raft::eraftpb::Message;
use anyhow::Result;
use raft::SnapshotStatus;
use crate::etcdserver::api::rafthttp::snap::message::SnapMessage;

pub trait Raft{
    fn process(&self, m: Message) -> Result<()>;
    fn is_id_removed(&self, id: u64) -> bool;
    fn report_unreachable(&self, id: u64);
    fn report_snapshot(&self, id: u64, status: SnapshotStatus);
}

pub trait Transporter{
    fn start(&self) -> Result<()>;
    fn handle(&self);
    fn send(&self, m:&[Message]) -> Result<()>;
    fn send_snapshot(&self, m: SnapMessage) -> Result<()>;
    fn add_remote(&self, id: u64, urls: Vec<String>) -> Result<()>;
    fn add_peer(&self, id: u64, urls: Vec<String>) -> Result<()>;
    fn remove_peer(&self, id: u64) -> Result<()>;
    fn remove_all_peers(&self) -> Result<()>;
    fn update_peer(&self, id: u64, urls: Vec<String>) -> Result<()>;
    fn active_since(&self) ->  std::time::SystemTime;
    fn active_peers(&self) -> i64;
    fn stop(&self) -> Result<()>;
}
