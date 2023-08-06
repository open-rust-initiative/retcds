use std::io::Error;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use futures::TryFutureExt;
use hyper::body::HttpBody;
use hyper::Request;
use openssl_sys::select;
use tokio::sync::Barrier;
use raft::eraftpb::Message;
use crate::etcdserver::api::rafthttp::{types};
use crate::etcdserver::api::rafthttp::http::RaftPrefix;
use crate::etcdserver::api::rafthttp::peer_status::PeerStatus;
use crate::etcdserver::api::rafthttp::transport::{Raft, Transport};
use crate::etcdserver::api::rafthttp::url_pick::urlPicker;
use crate::etcdserver::api::rafthttp::util::net_util::{check_post_response, create_POST_request, CustomError};
use crate::etcdserver::api::rafthttp::v2state::leader::FollowerStats;
use crate::etcdserver::async_ch::Channel;
use tokio::time::Instant;

const connPerPipeline: i32 = 4;
const pipelineBufSize: i32 = 64;

pub struct Pipeline {
    peer_id : types::id::ID,

    tr: Transport,
    picker : urlPicker,
    status : PeerStatus,
    raft : Box<dyn Raft>,

    errorc : Channel<Error>,

    follower_stats : FollowerStats,
    msgc : Channel<Message>,
    stopc : Channel<()>,
    wg: Arc<Mutex<usize>>,
}

impl Pipeline{
    fn start(&mut self){
        self.stopc = Channel::new(64);
        self.msgc = Channel::new(pipelineBufSize as usize);
        self.wg = Arc::new(Mutex::new(connPerPipeline as usize));
        for _ in 0..connPerPipeline{

        }

    }

    async fn handle(&mut self){
        loop{
            tokio::select!{
                _ = self.stopc.recv() => {
                    break;
                }
                msg = self.msgc.recv() => {
                    let start = Instant::now();
                }
            }
        }
    }

    async fn post(&self, data:Vec<u8>) -> Result<(),Error> {
        let u =self.picker.get_base_url_picker().lock().unwrap().pick().unwrap();
        let request = create_POST_request(u.clone(), RaftPrefix, data.clone(), "application/protobuf", self.tr.get_urls(), self.tr.get_id(), self.tr.get_cluster_id()).unwrap();
        let req = create_POST_request(u.clone(), RaftPrefix, data.clone(), "application/protobuf", self.tr.get_urls(), self.tr.get_id(), self.tr.get_cluster_id()).unwrap();

        let mut resp = self.tr.get_pipeline_client()
            .request(request)
            .await;
        if resp.is_err(){
            self.picker.get_base_url_picker().lock().unwrap().unreachable(u.clone());
            return Err(Error::new(std::io::ErrorKind::Other, resp.err().unwrap().to_string()));
        }

        // let vec = resp.as_ref().unwrap().data().await.unwrap().unwrap().to_vec();
        let err = check_post_response(resp.as_mut().unwrap(), req,self.peer_id.clone()).await;

        if err.is_err(){
            self.picker.get_base_url_picker().lock().unwrap().unreachable(u.clone());
            let temp_err = err.err().unwrap();
            let err_str = temp_err.to_string();
            if err_str.to_string().contains("the member has been permanently removed from the cluster") {
                self.errorc.send(temp_err).await.unwrap();
            }
            return Err(Error::new(std::io::ErrorKind::Other, err_str.clone()));
        }
        Ok(())
    }
}