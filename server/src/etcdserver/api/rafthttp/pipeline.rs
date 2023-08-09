use std::fmt::Debug;
use std::io::Error;
use std::ops::{Deref, Sub};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use actix::ActorStreamExt;
use chrono::Local;
use hyper::body::HttpBody;
use raft::eraftpb::Message;
use raft::eraftpb::MessageType;
use crate::etcdserver::api::rafthttp::{types};
use crate::etcdserver::api::rafthttp::http::RaftPrefix;
use crate::etcdserver::api::rafthttp::peer_status::{FailureType, PeerStatus};
use crate::etcdserver::api::rafthttp::transport::{Raft, Transport};
use crate::etcdserver::api::rafthttp::url_pick::urlPicker;
use crate::etcdserver::api::rafthttp::util::net_util::{check_post_response, create_POST_request, CustomError};
use crate::etcdserver::api::rafthttp::v2state::leader::FollowerStats;
use crate::etcdserver::async_ch::Channel;
use protobuf::Message as pMessage;
use slog::{info, warn};
use tracing::{event, Instrument, Level};
use tracing::subscriber::set_global_default;
// use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::Subscriber;
use tracing_subscriber::fmt::writer::MakeWriterExt;
use crate::etcdserver::api::rafthttp::peer::{is_msg_snap, pipelineMsg};
use crate::etcdserver::api::rafthttp::default_logger;


const connPerPipeline: i32 = 1;
const pipelineBufSize: i32 = 64;

#[derive(Debug)]
pub struct Pipeline {
    peer_id : types::id::ID,

    tr: Transport,
    picker : urlPicker,
    status : Arc<Mutex<PeerStatus>>,
    raft: Arc<Box<dyn Raft + Send + Sync>>,

    errorc : Channel<Error>,

    follower_stats: Arc<Mutex<FollowerStats>>,
    pub msgc : Channel<Message>,
    stopc : Channel<()>,
    done : Channel<()>
}

impl Clone for Pipeline{
    fn clone(&self) -> Self {
        Pipeline{
            peer_id : self.peer_id.clone(),
            tr : self.tr.clone(),
            picker : self.picker.clone(),
            status : self.status.clone(),
            raft : self.raft.clone(),
            errorc : self.errorc.clone(),
            follower_stats : self.follower_stats.clone(),
            msgc : self.msgc.clone(),
            stopc : self.stopc.clone(),
            done : self.done.clone(),
        }
    }
}

impl Pipeline{
    pub async fn start(&mut self){
        // tracing_subscriber::fmt::init();
        let subscriber = FmtSubscriber::builder()
            .with_max_level(Level::INFO)
            .finish();
        tracing::subscriber::set_global_default(subscriber)
            .expect("Failed to set subscriber");
        self.stopc = Channel::new(64);
        self.msgc = Channel::new(pipelineBufSize as usize);
        self.done = Channel::new(64);
        // self.wg = Arc::new(Mutex::new(connPerPipeline as usize));
        let peer_id = self.peer_id.to_string();
        let local_id = self.tr.get_id().to_string();
        let shared_self = Arc::new(self.clone());
        for _ in 0..connPerPipeline{
            let shared_self = Arc::clone(&shared_self);
            let res =  tokio::spawn(async move {
                shared_self.handle().await;
            });
        }
        info!(default_logger(),"started HTTP pipelining with remote peer remote-peer-id=>{}  local-member-id=>{}",peer_id,local_id);
    }

    pub async fn stop(&self){
        for _ in 0..connPerPipeline {
            self.stopc.send(()).await.unwrap();
        }

        info!(default_logger(),"stop HTTP pipelining with remote peer remote-peer-id=>{}  local-member-id=>{}",self.peer_id.to_string(),self.tr.get_id().to_string());
    }

    #[tracing::instrument]
    async fn handle(&self){
        loop{
           // tracing::warn!("into loop");
            tokio::select!{
                _ = self.stopc.recv() => {
                    return;
                }
                msg = self.msgc.recv() => {
                    let start = Instant::now();
                    let err = self.post(pMessage::write_to_bytes(&msg.clone().unwrap()).unwrap()).await;
                    let end = Instant::now();

                    if err.is_err(){
                        self.status.lock().unwrap().deactivate(FailureType::new_failure_type(pipelineMsg.to_string(), "write".to_string()),err.err().unwrap().to_string());
                        if msg.clone().unwrap().get_msg_type() == MessageType::MsgAppend {
                            self.follower_stats.lock().unwrap().fail();
                        }
                        self.raft.report_unreachable(msg.clone().unwrap().get_to());
                        if is_msg_snap(msg.clone().unwrap()){
                            self.raft.report_snapshot(msg.clone().unwrap().get_to(), raft::SnapshotStatus::Failure);
                        }
                        self.done.send(()).await.unwrap();
                        continue;
                    };
                    self.status.lock().unwrap().activate();
                    if msg.clone().unwrap().get_msg_type() == MessageType::MsgAppend {
                       self.follower_stats.lock().unwrap().succ(chrono::Duration::from_std(end.sub(start)).expect("trans time error"));
                    };
                    if is_msg_snap(msg.clone().unwrap()){
                        self.raft.report_snapshot(msg.clone().unwrap().get_to(), raft::SnapshotStatus::Finish);
                    };
                   self.done.send(()).await.unwrap();
                }
            }
        }
    }

    async fn post(&self, data:Vec<u8>) -> Result<(),Error> {
        let u =self.picker.get_base_url_picker().lock().unwrap().pick().unwrap();
        let request = create_POST_request(u.clone(), RaftPrefix, data.clone(), "application/protobuf", self.tr.get_urls(), self.tr.get_id(), self.tr.get_cluster_id()).unwrap();
        let req = create_POST_request(u.clone(), RaftPrefix, data.clone(), "application/protobuf", self.tr.get_urls(), self.tr.get_id(), self.tr.get_cluster_id()).unwrap();

        let mut resp = self.tr.pipeline_client.clone()
            .expect("no pipeline client")
            .request(request)
            .await;
        if resp.is_err(){
            self.picker.get_base_url_picker().lock().unwrap().unreachable(u.clone());
            return Err(Error::new(std::io::ErrorKind::Other, resp.err().unwrap().to_string()));
        }

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

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::future::Future;
    use std::io::Error;
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use hyper::{Body, Request, Response, Server, StatusCode};
    use hyper::server::conn::AddrIncoming;
    use hyper::service::{make_service_fn, service_fn};
    use hyper_rustls::TlsAcceptor;
    use openssl::x509::extension::ExtendedKeyUsage;
    use slog::{info, warn};
    use tokio::{io, select};
    use tracing::Instrument;
    use client::pkg::tlsutil::default_logger;
    use url::Url;
    use client::pkg::transport::listener::{new_tls_acceptor, self_cert, TLSInfo};
    use client::pkg::transport::transport::transport;
    use raft::eraftpb::{Message, MessageType};
    use raft::SnapshotStatus;
    use crate::etcdserver::api::rafthttp::peer_status::PeerStatus;
    use crate::etcdserver::api::rafthttp::pipeline::{connPerPipeline, Pipeline};
    use crate::etcdserver::api::rafthttp::test_util::{fakeRaft, new_tr, server_err, server_succ};
    use crate::etcdserver::api::rafthttp::transport::{Raft, Transport};
    use crate::etcdserver::api::rafthttp::types::id::ID;
    use crate::etcdserver::api::rafthttp::types::urls::URLs;
    use crate::etcdserver::api::rafthttp::url_pick::urlPicker;
    use crate::etcdserver::api::rafthttp::v2state::leader::FollowerStats;
    use crate::etcdserver::async_ch::Channel;

    #[tokio::test]
    async fn test_pipeline_send_err() {
        let urls = URLs::new(vec![Url::parse("https://localhost:2380").unwrap()]);
        let hosts = vec!["localhost"];
        let dirpath = "/tmp/test_self_cert";
        let self_signed_cert_validity = 365;
        let mut binding = ExtendedKeyUsage::new();
        let additional_usages = binding.client_auth();
        let info = self_cert(dirpath, hosts, self_signed_cert_validity, Some(additional_usages)).unwrap();
        let picker = urlPicker::new_url_picker(urls);
        let tr = new_tr("https://localhost:2380".to_string(),info.clone());
        server_err(info.clone()).await;
        let p = startTestPipeline(tr, picker).await;
        let mut m = Message::default();
        m.msg_type = MessageType::MsgAppend;
        p.msgc.send(m.clone()).await.unwrap();
        p.done.recv().await.unwrap();
        p.stop().await;
        assert_eq!(p.follower_stats.lock().unwrap().get_counts().get_fail(), 1);
    }

    #[tokio::test]
    async fn test_pipeline_send_succ() {
        let urls = URLs::new(vec![Url::parse("https://localhost:2380").unwrap()]);
        let hosts = vec!["localhost"];
        let dirpath = "/tmp/test_self_cert";
        let self_signed_cert_validity = 365;
        let mut binding = ExtendedKeyUsage::new();
        let additional_usages = binding.client_auth();
        let info = self_cert(dirpath, hosts, self_signed_cert_validity, Some(additional_usages)).unwrap();
        let picker = urlPicker::new_url_picker(urls);
        let tr = new_tr("https://localhost:2380".to_string(),info.clone());
        server_succ(info.clone()).await;
        let p = startTestPipeline(tr, picker).await;
        let mut m = Message::default();
        m.msg_type = MessageType::MsgAppend;
        p.msgc.send(m.clone()).await.unwrap();
        p.done.recv().await.unwrap();
        p.stop().await;
        assert_eq!(p.follower_stats.lock().unwrap().get_counts().get_success(), 1);
    }

    #[tokio::test]
    async fn test_pipeline_keep_send() {
        let urls = URLs::new(vec![Url::parse("https://localhost:2380").unwrap()]);
        let hosts = vec!["localhost"];
        let dirpath = "/tmp/test_self_cert";
        let self_signed_cert_validity = 365;
        let mut binding = ExtendedKeyUsage::new();
        let additional_usages = binding.client_auth();
        let info = self_cert(dirpath, hosts, self_signed_cert_validity, Some(additional_usages)).unwrap();
        let picker = urlPicker::new_url_picker(urls);
        let tr = new_tr("https://localhost:2380".to_string(),info.clone());
        server_succ(info.clone()).await;
        let p = startTestPipeline(tr, picker).await;
        let mut m = Message::default();
        m.msg_type = MessageType::MsgAppend;
        p.stop().await;
        for _ in 0..70{
            let result = p.msgc.try_send(m.clone()).await;
            if result.is_err(){
                info!(default_logger(),"send error");
            }
        }

        // assert_eq!(p.follower_stats.lock().unwrap().get_counts().get_success(), 50);
    }

    async fn startTestPipeline(tr: Transport, picker: urlPicker) -> Pipeline {
        let mut p = Pipeline {
            peer_id: ID::new(1),
            tr,
            picker,
            status: Arc::new(Mutex::new(PeerStatus::new(ID::new(1), ID::new(1)))),
            raft: Arc::new(Box::new(fakeRaft {
                recvc: Channel::new(1),
                err: "error".to_string(),
                removed_id: 1,
            })),
            errorc: Channel::new(10),
            follower_stats: Arc::new(Mutex::new(FollowerStats::default())),
            msgc: Channel::new(64),
            stopc: Channel::new(1),
            // wg : Channel::new(1),
            done: Channel::new(64),
        };
        p.start().await;
        return p;
    }
}