// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

// Copyright 2015 CoreOS, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// use harness::Network;
use protobuf::{Message as PbMessage, ProtobufEnum as _};
// use raft::eraftpb::*;
// use raft::storage::{GetEntriesContext, MemStorage};
// use raft::*;
use raft_proto::*;
use slog::Logger;
use raft_proto::eraftpb::{ConfChange, ConfChangeType, Entry, HardState, MessageType, Snapshot};
use crate::{Config, default_logger, raw_node, RawNode, SoftState, Storage, Ready, NO_LIMIT};
use crate::raw_node::SafeRawNode;
use crate::storage::MemStorage;
use env_logger::Env;
use std::io::Write;
// use crate::test_util::*;

pub fn conf_change(t: ConfChangeType, node_id: u64) -> ConfChange {
    let mut cc = ConfChange::default();
    cc.set_change_type(t);
    cc.node_id = node_id;
    cc
}

#[allow(clippy::too_many_arguments)]
pub fn must_cmp_ready(
    r: &Ready,
    ss: &Option<SoftState>,
    hs: &Option<HardState>,
    entries: &[Entry],
    committed_entries: &[Entry],
    snapshot: &Option<Snapshot>,
    msg_is_empty: bool,
    persisted_msg_is_empty: bool,
    must_sync: bool,
) {
    assert_eq!(r.ss(), ss.as_ref());
    assert_eq!(r.hs(), hs.as_ref());
    assert_eq!(r.entries().as_slice(), entries);
    assert_eq!(r.committed_entries().as_slice(), committed_entries);
    assert_eq!(r.must_sync(), must_sync);
    assert!(r.read_states().is_empty());
    assert_eq!(
        r.snapshot(),
        snapshot.as_ref().unwrap_or(&Snapshot::default())
    );
    assert_eq!(r.messages().is_empty(), msg_is_empty);
    assert_eq!(r.persisted_messages().is_empty(), persisted_msg_is_empty);
}

pub fn new_raw_node(
    id: u64,
    peers: Vec<u64>,
    election_tick: usize,
    heartbeat_tick: usize,
    storage: MemStorage,
    logger: &Logger,
) -> RawNode<MemStorage> {
    let config = new_test_config(id, election_tick, heartbeat_tick);
    new_raw_node_with_config(peers, &config, storage, logger)
}

pub fn new_safe_raw_node(
    id: u64,
    peers: Vec<u64>,
    election_tick: usize,
    heartbeat_tick: usize,
    storage: MemStorage,
    logger: &Logger,
) -> SafeRawNode<MemStorage> {
    let config = new_test_config(id, election_tick, heartbeat_tick);
    new_safe_raw_node_with_config(peers, &config, storage, logger)
}

pub fn new_raw_node_with_config(
    peers: Vec<u64>,
    config: &Config,
    storage: MemStorage,
    logger: &Logger,
) -> RawNode<MemStorage> {
    if storage.initial_state().unwrap().initialized() && peers.is_empty() {
        panic!("new_raw_node with empty peers on initialized store");
    }
    if !peers.is_empty() && !storage.initial_state().unwrap().initialized() {
        storage
            .wl()
            .apply_snapshot(new_snapshot(1, 1, peers))
            .unwrap();
    }
    RawNode::new(config, storage, logger).unwrap()
}


pub fn new_safe_raw_node_with_config(
    peers: Vec<u64>,
    config: &Config,
    storage: MemStorage,
    logger: &Logger,
) -> SafeRawNode<MemStorage> {
    if storage.initial_state().unwrap().initialized() && peers.is_empty() {
        panic!("new_raw_node with empty peers on initialized store");
    }
    if !peers.is_empty() && !storage.initial_state().unwrap().initialized() {
        storage
            .wl()
            .apply_snapshot(new_snapshot(1, 1, peers))
            .unwrap();
    }
    let raw_node = RawNode::new(config, storage, logger).unwrap();
    SafeRawNode::new(raw_node)
}

pub fn new_test_config(id: u64, election_tick: usize, heartbeat_tick: usize) -> Config {
    Config {
        id,
        election_tick,
        heartbeat_tick,
        max_size_per_msg: NO_LIMIT,
        max_inflight_msgs: 256,
        ..Default::default()
    }
}

pub fn new_snapshot(index: u64, term: u64, voters: Vec<u64>) -> Snapshot {
    let mut s = Snapshot::default();
    s.mut_metadata().index = index;
    s.mut_metadata().term = term;
    s.mut_metadata().mut_conf_state().voters = voters;
    s
}

#[cfg(any(test))]
pub(crate) fn try_init_log() {
    // env_logger::try_init_from_env(Env::new().default_filter_or("info"));
    let mut env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "trace");
    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} [{}:{}], {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.file().unwrap_or("<unnamed>"),
                record.line().unwrap(),
                &record.args()
            )
        })
        .try_init();
}
