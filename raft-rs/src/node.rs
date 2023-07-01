use std::fs::read;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};
use std::task::Context;
// use protobuf::Message;
use slog::{Drain, info, slog_warn, warn};
use crate::errors::{Error as RaftError, StorageError};
use raft_proto::ConfChangeI;
use raft_proto::eraftpb::{ConfChange, ConfChangeV2, ConfState, Entry, Message, MessageType};
use crate::{BaseStatus, Config, Peer, raft, RawNode, Ready, SnapshotStatus, storage, Storage};
use crate::storage::MemStorage;
use slog::{error, o};
use slog::Logger;
use async_channel::{self, bounded, unbounded, Receiver, Sender};
// use protobuf::Message;
use tokio::select;
// use tokio::sync::{RwLockReadGuard, RwLockWriteGuard};
use raft_proto::eraftpb::MessageType::{MsgHup, MsgPropose, MsgReadIndex, MsgSnapStatus, MsgTransferLeader, MsgUnreachable};

use crate::async_ch::{Channel, MsgWithResult};
use crate::raw_node::{is_local_msg, is_response_msg, SafeRawNode};
use crate::status::Status;


pub type SafeResult<T: Send + Sync + Clone> = Result<T, RaftError>;
#[derive(Clone)]
pub struct InnerChan<T> {
    rx: Option<Receiver<T>>,
    tx: Option<Sender<T>>,
}

impl<T> Default for InnerChan<T> {
    fn default() -> Self {
        let (tx, rx) = unbounded();
        InnerChan {
            tx: Some(tx),
            rx: Some(rx),
        }
    }
}

impl<T> InnerChan<T> {
    pub fn new() -> InnerChan<T> {
        let (tx, rx) = unbounded();
        InnerChan {
            tx: Some(tx),
            rx: Some(rx),
        }
    }

    pub fn new_with_cap(n: usize) -> InnerChan<T> {
        let (tx, rx) = bounded(n);
        InnerChan {
            tx: Some(tx),
            rx: Some(rx),
        }
    }

    pub fn new_with_channel(tx: Sender<T>, rx: Receiver<T>) -> InnerChan<T> {
        InnerChan {
            tx: Some(tx),
            rx: Some(rx),
        }
    }

    pub fn tx(&self) -> Sender<T> {
        self.tx.as_ref().unwrap().clone()
    }

    pub fn tx_ref(&self) -> &Sender<T> {
        self.tx.as_ref().unwrap()
    }

    pub fn rx(&self) -> Receiver<T> {
        self.rx.as_ref().unwrap().clone()
    }

    pub fn rx_ref(&self) -> &Receiver<T> {
        self.rx.as_ref().unwrap()
    }

    pub async fn try_send(&self, msg: T) -> Result<(), async_channel::SendError<T>> {
        if let Some(tx) = &self.tx {
            return tx.send(msg).await;
        }
        Ok(())
    }
}


#[derive(Clone)]
pub(crate) struct InnerNode<'a, S: Storage> {
    pub(crate) prop_c: Channel<MsgWithResult>,
    pub(crate) recv_c: Channel<Message>,
    conf_c: InnerChan<ConfChangeV2>,
    conf_state_c: InnerChan<ConfState>,
    ready_c: InnerChan<Ready>,
    advance: InnerChan<()>,
    tick_c: InnerChan<()>,
    done: Channel<()>,
    stop: InnerChan<()>,
    status: InnerChan<Sender<Status<'a>>>,
    raw_node: SafeRawNode<S>,
}

impl<S: Storage + Send + Sync + 'static> InnerNode<'_,S> {
    fn new(raw_node: SafeRawNode<S>) -> Self {
        InnerNode {
            prop_c: Channel::new(1),
            recv_c: Channel::new(1),
            conf_c: InnerChan::default(),
            conf_state_c: InnerChan::default(),
            ready_c: InnerChan::default(),
            advance: InnerChan::default(),
            tick_c: InnerChan::default(),
            done: Channel::new(1),
            stop: InnerChan::default(),
            status: InnerChan::default(),
            raw_node,
        }
    }



     async fn run(&mut self) {
        let mut wait_advance = false;
        let mut ready = Ready::default();
        let mut first = true;
        loop {
            {
                let mut has_ready = false;
                {
                    if !wait_advance && self.rl_raw_node().has_ready() && !first {
                        ready = self.rl_raw_node().ready_without_accept();
                        has_ready = true;
                    }
                }
                if has_ready {
                    self.ready_c.tx_ref().send(ready.clone()).await.unwrap();
                    self.wl_raw_node().accept_ready(&ready);
                    wait_advance = true;
                }
                first = false;
            }

            // pin_mut!(&self);
            select! {
                    conf = self.conf_c.rx_ref().recv() => {
                        let cc: ConfChangeV2 = conf.unwrap();
                        let mut cs = ConfState::new();
                        //  cs =raw_node.apply_conf_change(&cc).unwrap();
                        {
                             let mut raw_node = self.wl_raw_node();
                             let ok_before = raw_node.raft.prs.progress.contains_key(&raw_node.raft.id);
                             cs =raw_node.apply_conf_change(&cc).unwrap();
                             let (ok_after, id) = (raw_node.raft.prs.progress.contains_key(&raw_node.raft.id), raw_node.raft.id);
                             if ok_before && !ok_after {
                                   let _id = raw_node.raft.id;
                                   let found = cs.get_voters().iter().any(|id| *id == _id) || cs.get_voters_outgoing().iter().any(|id| *id == _id);
                                   if !found {
                                       println!("Current node({:#x}) isn't voter", id);
                                   }
                             }
                        }

                        select! {
                            _ = self.conf_state_c.tx_ref().send(cs) => {}
                            _ = self.done.recv() => {}
                        }
                    }
                    pm = self.prop_c.recv() => {
                        let mut pm: MsgWithResult = pm.unwrap();
                        if !self.is_voter() {
                            pm.notify(Err(RaftError::NotIsVoter)).await;
                        }else if !self.rl_raw_node().raft.has_leader() {
                            pm.notify(Err(RaftError::NoLeader)).await;
                        }else {
                              let mut msg: Message = pm.get_msg().unwrap().clone();
                              msg.set_from(self.rl_raw_node().raft.id);
                              let res = self.wl_raw_node().step(msg);
                              pm.notify_and_close(res).await;
                        }
                    }
                    msg = self.recv_c.recv() => {
                        let msg: Message = msg.unwrap();
                        // filter out response message from unknown From.
                        let mut raw_node = self.wl_raw_node();
                        let is_pr = raw_node.raft.prs.progress.contains_key(&msg.get_from());
                        if is_pr || !is_response_msg(msg.get_msg_type()) {
                           raw_node.raft.step(msg);
                        }
                    }
                    _ = self.tick_c.rx_ref().recv() => {
                        self.tick().await;
                    }
                    _ = self.advance.rx_ref().recv() => {
                        {
                            self.wl_raw_node().advance(ready.clone());
                            wait_advance = false;
                        }
                    }

                    _ = self.stop.rx_ref().recv() => {
                        if let Some(tx) = self.done.take_tx() {
                            tx.send(()).await;
                        }
                        return;
                    }

                    status = self.status.rx_ref().recv() => {
                            let _status = Status::from(&self.raw_node.rl().raft);
                            status.unwrap().send(_status).await;
                }
            }
        }
    }

    async fn do_step(&self, m: Message) {
        self.step_wait_option(m,false).await;
    }

    async fn step_wait(&self, m: Message) -> SafeResult<()> {
        self.step_wait_option(m, true).await
    }

    async fn step_wait_option(&self, m: Message, wait: bool) -> SafeResult<()>{
        if m.get_msg_type() != MsgPropose {
            select! {
                _= self.recv_c.send(m.clone()) =>  return Ok(()),
                _= self.done.recv() => {
                    return Err(RaftError::Stopped)
                }
            }
        }
        let ch = self.prop_c.tx();
        let mut notify = Channel::new(1);
        let mut props_msg = if !wait {
            MsgWithResult::new_with_msg(m.clone())
        } else {
            MsgWithResult::new_with_channel(notify.tx(), m.clone())
        };

        select! {
            _= ch.send(props_msg) => {
                if !wait{
                    return Ok(())
                }
            }
            _= self.done.recv() => {
                return Err(RaftError::Stopped)
            }
        }
        select! {
            res = notify.recv() => {
                return res.unwrap()
            }
            _= self.done.recv() => {
                return Err(RaftError::Stopped)
            }
        }
    }

    fn get_status(&self) -> Status{
        Status::from(&self.raw_node.rl().raft)
    }

    pub fn rl_raw_node_fn<F>(&self,mut f:F)
        where
            F: FnMut(RwLockReadGuard<'_, RawNode<S>>),
        {
            let rl =self.rl_raw_node();
            f(rl)
        }

    pub fn wl_raw_node_fn<F>(&self, mut f: F)
        where
            F: FnMut(RwLockWriteGuard<'_, RawNode<S>>),
    {
        let wl = self.wl_raw_node();
        f(wl)
    }

    pub fn rl_raw_node(&self) -> RwLockReadGuard<'_, RawNode<S>> {
        self.raw_node.rl()
    }

    pub fn wl_raw_node(&self) -> RwLockWriteGuard<'_, RawNode<S>> {
        self.raw_node.wl()
    }

    fn is_voter(&self) -> bool {
        let raw_node = self.rl_raw_node();
        let _id = raw_node.raft.id;
        let cs = raw_node.raft.prs.conf.to_conf_state();
        cs.get_voters().iter().any(|id| *id == _id)
            || cs.get_voters_outgoing().iter().any(|id| *id == _id)
    }

    fn is_voter_with_conf_state(&self, cs: &ConfState) -> bool {
        let raw_node = self.rl_raw_node();
        let _id = raw_node.raft.id;
        cs.get_voters().iter().any(|id| *id == _id)
            || cs.get_voters_outgoing().iter().any(|id| *id == _id)
    }

}





use async_trait::async_trait;
use bytes::Bytes;
use futures::future::ok;

use protobuf::RepeatedField;

#[async_trait]
pub trait Node {
    /// Increments the interval logical clock for the `Node` by a single tick. Election
    /// timeouts and heartbeat timeouts are in units of ticks.
    async fn tick(&self);

    /// Causes the `Node` to transition to candidate state and start campaign to become leader.
    async fn campaign(&self) -> SafeResult<()>;

    /// proposes that data be appended to the log. Note that proposals can be lost without
    /// notice, therefore it is user's job to ensure proposal retries.
    async fn propose(&self, data: &[u8]) -> SafeResult<()>;

    /// Proposes a configuration change. Like any proposal, the
    /// configuration change may be dropped with or without an error being
    /// returned. In particular, configuration changes are dropped unless the
    /// leader has certainty that there is no prior unapplied configuration
    /// change in its log.
    ///
    /// The method accepts either a pb.ConfChange (deprecated) or pb.ConfChangeV2
    /// message. The latter allows arbitrary configuration changes via joint
    /// consensus, notably including replacing a voter. Passing a ConfChangeV2
    /// message is only allowed if all Nodes participating in the cluster run a
    /// version of this library aware of the V2 API. See pb.ConfChangeV2 for
    /// usage details and semantics.
    async fn propose_conf_change(&self, cc: impl ConfChangeI + Send) -> SafeResult<()>;

    /// Step advances the state machine using the given message. ctx.Err() will be returned, if any.
    async fn step(&self, msg: Message) -> SafeResult<()>;

    /// Ready returns a channel that returns the current point-in-time state.
    /// Users of the Node must call Advance after retrieving the state returned by Ready.
    ///
    /// NOTE: No committed entries from the next Ready may be applied until all committed entries
    /// and snapshots from the previous one have finished.
    fn ready(&self) -> Receiver<Ready>;

    /// Advance notifies the Node that the application has saved progress up to the last Ready.
    /// It prepares the node to return the next available Ready.
    ///
    /// The application should generally call Advance after it applies the entries in last Ready.
    ///
    /// However, as an optimization, the application may call Advance while it is applying the
    /// commands. For example. when the last Ready contains a snapshot, the application might take
    /// a long time to apply the snapshot data. To continue receiving Ready without blocking raft
    /// progress, it can call Advance before finishing applying the last ready.
    async fn advance(&self);

    /// ApplyConfChange applies a config change (previously passed to
    /// ProposeConfChange) to the node. This must be called whenever a config
    /// change is observed in Ready.CommittedEntries, except when the app decides
    /// to reject the configuration change (i.e. treats it as a noop instead), in
    /// which case it must not be called.
    ///
    /// Returns an opaque non-nil ConfState protobuf which must be recorded in
    /// snapshots.
    async fn apply_conf_change(&self, cc: ConfChange) -> Option<ConfState>;

    /// TransferLeadership attempts to transfer leadership to the given transferee.
    async fn transfer_leader_ship(&self, lead: u64, transferee: u64);

    /// ReadIndex request a read state. The read state will be set in the ready.
    /// Read state has a read index. Once the application advances further than the read
    /// index, any linearize read requests issued before the read request can be
    /// processed safely. The read state will have the same rctx attached.
    async fn read_index(&self, rctx: Vec<u8>) -> SafeResult<()>;

    /// Status returns the current status of the raft state machine.
    async fn status(&self) -> Status;

    /// reports the given node is not reachable for the last send.
    async fn report_unreachable(&self, id: u64);

    /// reports the status of the sent snapshot. The id is the raft `ID` of the follower
    /// who is meant to receive the snapshot, and the status is `SnapshotFinish` or `SnapshotFailure`.
    /// Calling `ReportSnapshot` with `SnapshotFinish` is a no-op. But, any failure in applying a
    /// snapshot (for e.g., while streaming it from leader to follower), should be reported to the
    /// leader with SnapshotFailure. When leader sends a snapshot to a follower, it pauses any raft
    /// log probes until the follower can apply the snapshot and advance its state. If the follower
    /// can't do that, for e.g., due to a crash, it could end up in a limbo, never getting any
    /// updates from the leader. Therefore, it is crucial that the application ensures that any
    /// failure in snapshot sending is caught and reported back to the leader; so it can resume raft
    /// log probing in the follower.
    async fn report_snapshot(&self, id: u64, status: SnapshotStatus);

    /// performs any necessary termination of the `Node`.
    async fn stop(&self);
}

#[async_trait]
impl<S: Storage + Send + Sync + 'static> Node for InnerNode<'_,S>{
    async fn tick(&self) {
        self.raw_node.wl().tick();
    }

    async fn campaign(&self) -> SafeResult<()> {
        let mut msg =Message::new();
        msg.set_msg_type(MsgHup);
        self.do_step(msg).await;
        Ok(())
    }

   async fn propose(&self, data: &[u8]) -> SafeResult<()> {
        let msg =Message{
            msg_type:MsgPropose,
            entries:RepeatedField::from(vec![Entry{
                data: Bytes::from(data.to_vec()),
                ..Default::default()
            }]),
            ..Default::default()
        };
        self.step_wait(msg).await;
        Ok(())
    }

    async fn propose_conf_change(&self, cc: impl ConfChangeI+ Send) -> SafeResult<()> {
        let mut entry = cc.to_entry();
        let mut msg = Message::new();
        msg.set_msg_type(MsgPropose);
        msg.set_entries(RepeatedField::from(vec![entry]));
        self.step(msg).await?;
        Ok(())

    }

    async fn step(&self, msg: Message) -> SafeResult<()> {

        // if msg.msg_type == MessageType::MsgHup {
        //     return Ok(());
        // }

        if is_local_msg(msg.get_msg_type()) {
            return Ok(());
        }
        self.do_step(msg).await;
        Ok(())
    }

    fn ready(&self) -> Receiver<Ready> {
        self.ready_c.rx()
    }

    async fn advance(&self) {
        let advance = self.advance.tx();
        select! {
            _= advance.send(()) =>{

                // info!("execute advance");
                println!("execute advance");
            },
            _=self.done.recv() =>{}
        }
    }

    async fn apply_conf_change(&self, cc: ConfChange) -> Option<ConfState> {
        let cc_v2 =cc.as_v2();
        let conf_tx =self.conf_c.tx();
        select! {
            _= conf_tx.send(cc_v2.as_ref().clone()) =>{}
            _= self.done.recv() =>{}
        }
        let conf_state =self.conf_state_c.rx();
        select! {
           res =conf_state.recv() => Some(res.unwrap()),
            _=self.done.recv() =>None
        }
    }

    async fn transfer_leader_ship(&self, lead: u64, transferee: u64) {
        let recvc =self.recv_c.tx();
        let mut msg = Message::new();
        msg.set_msg_type(MsgTransferLeader);
        msg.set_from(transferee);
        msg.set_to(lead);
        select! {
            _= recvc.send(msg) =>{}
            _= self.done.recv() =>{}
        }
    }

    async fn read_index(&self, rctx: Vec<u8>) -> SafeResult<()> {
        let mut msg =Message::new();
        msg.set_msg_type(MsgReadIndex);
        let mut entry = Entry::default();
        entry.set_data(Bytes::from(rctx));
        msg.set_entries(RepeatedField::from(vec![entry]));
        self.step(msg).await
    }

    async fn status(&self) -> Status {
        let status = self.status.tx();
        let ch:InnerChan<Status> =InnerChan::new();
        let (tx,rx)=(ch.tx(),ch.rx());
        select! {
            _= status.send(tx) =>{}
            _= self.done.recv() =>{}
        }
        rx.recv().await.unwrap()
    }

    async fn report_unreachable(&self, id: u64) {
        let recv =self.recv_c.tx();
        let mut msg =Message::new();
        msg.set_msg_type(MsgUnreachable);
        msg.set_from(id);
        select! {
            _= recv.send(msg) =>{}
            _= self.done.recv() =>{}
        }
    }

    async fn report_snapshot(&self, id: u64, status: SnapshotStatus) {
        let recv =self.recv_c.tx();
        let rejected =status == SnapshotStatus::Failure;
        let mut msg =Message::new();
        msg.set_msg_type(MsgSnapStatus);
        msg.set_from(id);
        msg.set_reject(rejected);
        select! {
            _= recv.send(msg) =>{}
            _= self.done.recv() =>{}
        }
    }

    async fn stop(&self) {
        let stop = self.stop.tx();
        select! {
            _= stop.send(()) =>{},
            _= self.done.recv() => return
        }
        self.done.recv().await;
    }
}

pub(crate) async fn start_node<T:Storage+Send + Sync + Clone + 'static>(
    config: Config,
    mut store: T,
    peers:Vec<Peer>
) -> InnerNode<'static,T> {
    assert!(!peers.is_empty(), "no peers given; use RestartNode instead");
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain)
        .chan_size(4096)
        .overflow_strategy(slog_async::OverflowStrategy::Block)
        .build()
        .fuse();
    let default_logger = slog::Logger::root(drain, o!("tag" => format!("[{}]", 1)));


    let mut r = RawNode::new(&config, store, &default_logger).unwrap();
    r.bootstrap(peers).expect("TODO: panic message");

    let mut node = InnerNode::new(SafeRawNode::new(r));
    let mut node1 = node.clone();
    tokio::spawn(async move {
        node1.run().await;
    });
    node
}

#[cfg(test)]
mod tests {
    use protobuf::ProtobufEnum;
    use slog::{info, log, o};
    // use tokio::net::windows::named_pipe::PipeMode::Message;
    use raft_proto::eraftpb::{Message, MessageType};
    use raft_proto::eraftpb::MessageType::{MsgPropose, MsgRequestPreVoteResponse};
    use crate::async_ch::Channel;
    use crate::node::{InnerChan, InnerNode, Node};
    use crate::raw_node::is_local_msg;
    use crate::{RawNode, ReadState};
    use lazy_static::lazy_static;
    use crate::storage::MemStorage;
    use std::sync::{Arc, Mutex};
    use env_logger::Env;
    use protobuf::descriptorx::MessageOrEnumWithScope;
    use crate::test_util::{new_raw_node, new_safe_raw_node, try_init_log};
    use slog::Drain;
    // use crate::tests_util::try_init_log;
    lazy_static! {
        /// This is an example for using doc comment attributes
        static ref msgs: Arc<Mutex<Vec<Message>>> = Arc::new(Mutex::new(vec ! []));
    }

    #[test]
    fn t_drop() {
        tokio::runtime::Runtime::new().unwrap().block_on(async move {
            let mut ch: InnerChan<usize> = InnerChan::new();
            let rx = ch.rx.take();
            drop(rx);
            let tx = ch.tx();
            let res = tx.send(19).await;
            assert!(res.is_err());
        });
    }

    // ensures that node.step sends msgProp to propc chan
    // and other kinds of messages to recvc chan.
    #[tokio::test]
    async fn t_node_step() {
        try_init_log();
        for msgn in 0..MsgRequestPreVoteResponse.value() {
            let mut node: InnerNode<MemStorage> =
                InnerNode::new(new_safe_raw_node(1, vec![1], 10, 1, MemStorage::new(), &slog::Logger::root(slog::Discard, o!())));
            node.prop_c = Channel::new(1);
            node.recv_c = Channel::new(1);
            let msgt = MessageType::from_i32(msgn).unwrap();
            let mut msg = Message::new();
            msg.set_msg_type(msgt);

            let ok = node.step(msg.clone()).await;

            assert!(ok.is_ok(), "{:?}", ok.unwrap_err());

            if msgt == MsgPropose {
                let proposal_rx = node.prop_c.recv().await;
                assert!(
                    proposal_rx.is_ok(),
                    "{}: cannot receive {:?} on propc chan",
                    msgn,
                    msgt
                );
            } else {
                if is_local_msg(msgt) {
                    assert!(
                        node.recv_c.try_recv().await.is_err(),
                        "{}: step should ignore {:?}",
                        msgn,
                        msgt
                    );
                } else {
                    assert!(
                        node.recv_c.try_recv().await.is_ok(),
                        "{}: cannot receive {:?} on recvc chan",
                        msgn,
                        msgt
                    );
                }
            }
        }
    }

    #[tokio::test]
    async fn t_node_process() {
        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain)
            .chan_size(4096)
            .overflow_strategy(slog_async::OverflowStrategy::Block)
            .build()
            .fuse();
        let default_logger = slog::Logger::root(drain, o!("tag" => format!("[{}]", 1)));
        try_init_log();
        {
            msgs.lock().unwrap().clear();
        }
        let s = MemStorage::new();
        let raw_node = new_safe_raw_node(1, vec![1], 10, 1, s.clone(), &default_logger);
        let mut node= InnerNode::<MemStorage>::new(raw_node);
        let mut node1 = node.clone();
        tokio::spawn(async move { node1.run().await });
        let ok = node.campaign().await;
        assert!(ok.is_ok(), "{:?}", ok.unwrap_err());
        loop {
            let rd = node.ready().recv().await.unwrap();
            println!("get a ready_rx, wait it: {:?}",rd);
            s.wl().append(rd.entries());
            if rd.ss().as_ref().unwrap().leader_id == node.rl_raw_node().raft.leader_id {
                node.advance().await;
                break;
            }
            node.advance().await;
        }

        assert!(node.propose("somedata".as_bytes()).await.is_ok());
        node.stop().await;
        println!("mail-box: {:?}", node.raw_node.rl().raft.msgs)
    }

    #[tokio::test]
    async fn t_node_read_index() {
        try_init_log();

        let decorator = slog_term::TermDecorator::new().build();
        let drain = slog_term::FullFormat::new(decorator).build().fuse();
        let drain = slog_async::Async::new(drain)
            .chan_size(4096)
            .overflow_strategy(slog_async::OverflowStrategy::Block)
            .build()
            .fuse();
        let default_logger = slog::Logger::root(drain, o!("tag" => format!("[{}]", 1)));

        let s = MemStorage::new();
        let raw_node = new_safe_raw_node(1, vec![1], 10, 1, s.clone(), &default_logger);
        let mut node = InnerNode::<MemStorage>::new(raw_node);
        let wrs = vec![ReadState {
            index: 1,
            request_ctx: "somedata".as_bytes().to_vec(),
        }];

        {
            node.wl_raw_node().raft.read_states = wrs.clone();
        }

        let mut node1 = node.clone();
        tokio::spawn(async move { node1.run().await });

        let ok = node.campaign().await;
        assert!(ok.is_ok(), "{:?}", ok.unwrap_err());

        loop {
            println!("try again");
            let ready = node.ready_c.rx();
            let ready = ready.recv().await.unwrap();

            {
                let mut raw_node = node.wl_raw_node();
                let expect = ready.read_states().clone();
                assert_eq!(expect, wrs);
                s.wl().append(ready.entries());

                if ready.ss().as_ref().unwrap().leader_id == raw_node.raft.id {
                    node.advance();
                    break;
                }
            }

            node.advance();
        }

        let w_request = "somedata2".as_bytes().to_vec();
        node.read_index(w_request.clone());
        assert!(node.propose("somedata".as_bytes()).await.is_ok());
        println!("is");
        // dbg!(x);
        node.stop();
    }
}
