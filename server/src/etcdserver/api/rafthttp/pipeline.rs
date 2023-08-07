use std::io::Error;
use std::ops::{Deref, Sub};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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
use slog::info;
use crate::etcdserver::api::rafthttp::peer::{is_msg_snap, pipelineMsg};
use crate::etcdserver::api::rafthttp::default_logger;

const connPerPipeline: i32 = 4;
const pipelineBufSize: i32 = 64;

pub struct Pipeline {
    peer_id : types::id::ID,

    tr: Transport,
    picker : urlPicker,
    status : PeerStatus,
    raft: Arc<Box<dyn Raft + Send + Sync>>,

    errorc : Channel<Error>,

    follower_stats : FollowerStats,
    msgc : Channel<Message>,
    stopc : Channel<()>,
    wg: Arc<Mutex<usize>>,
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
            wg : self.wg.clone(),
        }
    }
}

impl Clone for Box<dyn Raft +Send +Sync+'static>{
    fn clone(&self) -> Self {
        self.clone()
    }
}

// impl Clone for

impl Pipeline{
    async fn start(&self){
        // self.stopc = Channel::new(64);
        // self.msgc = Channel::new(pipelineBufSize as usize);
        // self.wg = Arc::new(Mutex::new(connPerPipeline as usize));
        let peer_id = self.peer_id.to_string();
        let local_id = self.tr.get_id().to_string();
        let shared_self = Arc::new(self.clone());
        for _ in 0..connPerPipeline{
            let shared_self = Arc::clone(&shared_self);
            tokio::spawn(async move {
                shared_self.handle().await;
            });
        }
        info!(default_logger(),"started HTTP pipelining with remote peer remote-peer-id=>{}  local-member-id=>{}",peer_id,local_id);
    }

    async fn stop(&self){
        self.stopc.send(()).await.unwrap();

        info!(default_logger(),"started HTTP pipelining with remote peer remote-peer-id=>{}  local-member-id=>{}",self.peer_id.to_string(),self.tr.get_id().to_string());
    }

    async fn handle(&self){
        loop{
            tokio::select!{
                _ = self.stopc.recv() => {
                    return;
                }
                msg = self.msgc.recv() => {
                    let start = Instant::now();
                    let err = self.post(pMessage::write_to_bytes(&msg.clone().unwrap()).unwrap()).await;
                    let end = Instant::now();
                    if err.is_err(){
                        self.status.get_base_peer_status().lock().await.deactivate(FailureType::new_failure_type(pipelineMsg.to_string(), "write".to_string()),err.err().unwrap().to_string());
                        if msg.clone().unwrap().get_msg_type() == MessageType::MsgAppend {
                            self.follower_stats.fail()
                        }
                        self.raft.report_unreachable(msg.clone().unwrap().get_to());
                        if is_msg_snap(msg.clone().unwrap()){
                            self.raft.report_snapshot(msg.clone().unwrap().get_to(), raft::SnapshotStatus::Failure);
                        }
                        continue;
                    }
                    self.status.get_base_peer_status().lock().await.activate();
                    if msg.clone().unwrap().get_msg_type() == MessageType::MsgAppend {
                        self.follower_stats.succ(chrono::Duration::from_std(end.sub(start)).expect("trans time error"));
                    }
                    if is_msg_snap(msg.clone().unwrap()){
                        self.raft.report_snapshot(msg.clone().unwrap().get_to(), raft::SnapshotStatus::Finish);
                    };
                }
            }
        }
    }

    async fn post(&self, data:Vec<u8>) -> Result<(),Error> {
        let u =self.picker.get_base_url_picker().lock().unwrap().pick().unwrap();
        let request = create_POST_request(u.clone(), RaftPrefix, data.clone(), "application/protobuf", self.tr.get_urls(), self.tr.get_id(), self.tr.get_cluster_id()).unwrap();
        let req = create_POST_request(u.clone(), RaftPrefix, data.clone(), "application/protobuf", self.tr.get_urls(), self.tr.get_id(), self.tr.get_cluster_id()).unwrap();

        let mut resp = self.tr.get_pipeline_client()
            .expect("no pipeline client")
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

#[cfg(test)]
mod tests {
    use std::io::Error;
    use std::sync::{Arc, Mutex};
    use async_trait::async_trait;
    use hyper::{Body, Response, Server};
    use hyper::server::conn::AddrIncoming;
    use hyper::service::{make_service_fn, service_fn};
    use hyper_rustls::TlsAcceptor;
    use openssl::x509::extension::ExtendedKeyUsage;
    use slog::warn;
    use tokio::{io, select};
    use url::Url;
    use client::pkg::transport::listener::{new_tls_acceptor, self_cert, TLSInfo};
    use client::pkg::transport::transport::transport;
    use raft::eraftpb::Message;
    use raft::SnapshotStatus;
    use crate::etcdserver::api::rafthttp::peer_status::PeerStatus;
    use crate::etcdserver::api::rafthttp::pipeline::{connPerPipeline, Pipeline};
    use crate::etcdserver::api::rafthttp::transport::{Raft, Transport};
    use crate::etcdserver::api::rafthttp::types::id::ID;
    use crate::etcdserver::api::rafthttp::types::urls::URLs;
    use crate::etcdserver::api::rafthttp::url_pick::urlPicker;
    use crate::etcdserver::api::rafthttp::v2state::leader::FollowerStats;
    use crate::etcdserver::async_ch::Channel;

    #[tokio::test]
    async fn test(){
        let urls = URLs::new(vec![Url::parse("http://localhost:2380").unwrap()]);
        let hosts = vec!["localhost"];
        let dirpath = "/tmp/test_self_cert";
        let self_signed_cert_validity = 365;
        let mut binding = ExtendedKeyUsage::new();
        let additional_usages = binding.client_auth();
        let info = self_cert(dirpath, hosts, self_signed_cert_validity, Some(additional_usages)).unwrap();
        let picker = urlPicker::new_url_picker(urls);
        let tr = Transport::new(
            vec!["http://localhost:2380".to_string()],
            None,
            std::time::Duration::from_secs(1),
            0.1,
            1,
            1,
            None,
            None,
            None,
            None,
            Option::from(transport(info.clone())),
        );

        // tokio::spawn(async move {
            startTestPipeline(tr,picker).await;
        // });
        // tokio::spawn({
        //     startTestPipeline(tr,picker).await;
        // });
        server(info.clone()).await;
        let client = transport(info);
        let response = client.get("https://localhost:2380".parse().unwrap()).await.unwrap();
        assert!(response.status().is_success());

    }

    async fn startTestPipeline(tr:Transport, picker:urlPicker) -> Pipeline {
        let mut p = Pipeline{
            peer_id : ID::new(1),
            tr,
            picker,
            status : PeerStatus::new_peer_status(ID::new(1),ID::new(1)),
            raft : Arc::new(Box::new(fakeRaft{
                recvc : Channel::new(1),
                err : "error".to_string(),
                removed_id : 1,
            })),
            errorc : Channel::new(1),
            follower_stats : FollowerStats::default(),
            msgc : Channel::new(1),
            stopc : Channel::new(1),
            // wg : Channel::new(1),
            wg: Arc::new(Mutex::new(connPerPipeline as usize)),
        };
        // tokio::spawn({
            p.start().await;
        // });


        return p;
    }

    struct fakeRaft{
        recvc : Channel<Message>,
        err : String,
        removed_id : u64,
    }


    #[async_trait]
    impl Raft for fakeRaft{
        async fn process(&self, m: Message) -> Result<(),Error> {
            select!{
                msg = self.recvc.recv() =>{},
            }
            return Err(Error::new(std::io::ErrorKind::Other, self.err.clone()));
        }

        fn is_id_removed(&self, id: u64) -> bool {
            return id == self.removed_id;
        }

        fn report_unreachable(&self, id: u64) {
            todo!()
        }

        fn report_snapshot(&self, id: u64, status: SnapshotStatus) {
            todo!()
        }
    }

    async fn server(tlsinfo:TLSInfo){
        let addr = "127.0.0.1:2380".parse().unwrap();

        let incoming = AddrIncoming::bind(&addr).unwrap();
        let tls_acceptor = new_tls_acceptor(tlsinfo.clone());

        let acceptor = TlsAcceptor::builder()
            .with_tls_config((*tlsinfo.clone().server_config()).clone())
            .with_all_versions_alpn()
            .with_incoming(incoming);
        let service = make_service_fn(|_| async { Ok::<_, io::Error>(service_fn(|_req|async {Ok::<_, io::Error>(Response::new(Body::empty()))})) });

        tokio::spawn(async move {
            let server = Server::builder(acceptor)
                .http2_only(true)
                .serve(service);
            server.await.unwrap();
        });
    }
}