use std::io::Error;
use std::sync::{Arc, Mutex};
use tokio::sync::Barrier;
use raft::eraftpb::Message;
use crate::etcdserver::api::rafthttp::{types};
use crate::etcdserver::api::rafthttp::peer_status::PeerStatus;
use crate::etcdserver::api::rafthttp::transport::Raft;
use crate::etcdserver::api::rafthttp::url_pick::urlPicker;
use crate::etcdserver::api::rafthttp::v2state::leader::FollowerStats;
use crate::etcdserver::async_ch::Channel;

const connPerPipeline: i32 = 4;
const pipelineBufSize: i32 = 64;

pub struct Pipeline {
    peer_id : types::id::ID,

    picker : urlPicker,
    status : PeerStatus,
    raft : Box<dyn Raft>,

    errorc : Channel<Error>,

    follower_stats : FollowerStats,
    msgc : Channel<Message>,
    stopc : Channel<()>,
    wg: Arc<Mutex<Barrier>>,
}

impl Pipeline{

}