use std::sync::{Arc, Mutex};
use slog::warn;

use raft::eraftpb::Message;
use crate::etcdserver::api::rafthttp::peer_status::PeerStatus;
use crate::etcdserver::api::rafthttp::pipeline::Pipeline;
use crate::etcdserver::api::rafthttp::types::id::ID;
use crate::etcdserver::api::rafthttp::default_logger;
use protobuf::ProtobufEnum;

pub struct remote{
    local_id : ID,
    id : ID,
    status : Arc<Mutex<PeerStatus>>,
    pipeline : Pipeline,
}

impl remote{
    pub fn new(local_id: ID, id: ID, status: Arc<Mutex<PeerStatus>>, pipeline: Pipeline) -> remote {
        remote {
            local_id,
            id,
            status,
            pipeline,
        }
    }

    pub  async fn send(&self,m:Message){
        match self.pipeline.msgc.try_send(m.clone()).await{
            Ok(()) => {},
            Err(e) => {
                if self.status.lock().unwrap().is_active(){
                    warn!(default_logger(),"dropped internal Raft message since sending buffer is full (overloaded network)");
                    warn!(default_logger(),"message-type => {} local-member-id => {} from => {} remote-peer-id => {} remote-peer-active=> {}",
                        ProtobufEnum::value(&m.clone().get_msg_type()),
                        self.local_id.to_string(),
                        m.get_from(),
                        self.id.to_string(),
                        self.status.lock().unwrap().is_active());
                }
                else {
                    warn!(default_logger(),"dropped Raft message since sending buffer is full (overloaded network)");
                    warn!(default_logger(),"message-type => {} local-member-id => {} from => {} remote-peer-id => {} remote-peer-active=> {}",
                        ProtobufEnum::value(&m.clone().get_msg_type()),
                        self.local_id.to_string(),
                        m.get_from(),
                        self.id.to_string(),
                        self.status.lock().unwrap().is_active());
                }
            }
        }
    }

    pub fn stop(&self){
        self.pipeline.stop();
    }

    pub fn pause(&self){
        self.stop();
    }

    pub fn resume(&mut self){
        self.pipeline.start();
    }
}