use std::env::Args;
use actix::Handler;
use actix_ratelimit::ActorMessage;
use actix_web::{HttpServer};
use raft::eraftpb::Message;
use anyhow::Result;
use raft::SnapshotStatus;
use client::pkg::transport::listener::TLSInfo;
use crate::etcdserver::api::rafthttp::snap::message::SnapMessage;
use crate::etcdserver::api::rafthttp::snap::snap_shotter::SnapShotter;
use crate::etcdserver::api::rafthttp::types::id::ID;
use crate::etcdserver::api::rafthttp::types::urls::URLs;
use crate::etcdserver::api::rafthttp::v2state::leader::LeaderStats;
use crate::etcdserver::api::rafthttp::v2state::server::ServerState;

pub trait Raft{
    fn process(&self, m: Message) -> Result<()>;
    fn is_id_removed(&self, id: u64) -> bool;
    fn report_unreachable(&self, id: u64);
    fn report_snapshot(&self, id: u64, status: SnapshotStatus);
}

pub trait Transporter{
    fn start(&self) -> Result<()>;
    // fn handle(&self) -> Result<HttpServer<F, I, S, B>>;
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

pub struct Transport{
    logger : slog::Logger,

    tlsinfo: TLSInfo,

    dial_timeout : std::time::Duration,
    dial_retry_frequency : f64,

    ID : ID,
    URLS : URLs,
    cluster_id : ID,
    snap_shotter : SnapShotter,

    server_stats : ServerState,

    leader_stats : LeaderStats,



}
