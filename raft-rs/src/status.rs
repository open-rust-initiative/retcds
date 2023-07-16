// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

// Copyright 2015 The etcd Authors
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

use crate::eraftpb::HardState;
use crate::{ ProgressTracker};

use crate::raft::{Raft, SoftState, StateRole};
use crate::storage::Storage;
// use crate::{Config, ProgressTracker};
use crate::tracker::ProgressMap;
use crate::tracker::Configuration;
use std::fmt::{Display, Formatter};

/// Contains information about this Raft peer and its view of the system.
/// The Progress is only populated on the leader.
#[derive(Debug)]
pub struct Status<'a> {
    pub(crate) base_status: BaseStatus<'a>,
    pub config: Configuration,
    pub progress: ProgressMap,
}

impl Display for Status<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> ::std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl<'a> Status<'a> {
    pub(crate) fn New<T: Storage>(raft: &'a Raft<T>) -> Status<'a> {
        let mut s = Status {
            base_status: BaseStatus::new(raft),
            config: Default::default(),
            progress: Default::default(),
        };
        if s.base_status.ss.raft_state == StateRole::Leader {
            s.progress = raft.prs.progress.clone();
        }
        s.config = raft.prs.conf.clone();
        s
    }
}


/// Represents the current status of the raft
#[derive(Default,Debug)]
pub struct BaseStatus<'a> {
    /// The ID of the current node.
    pub id: u64,
    /// The hardstate of the raft, representing voted v2state.
    pub hs: HardState,
    /// The softstate of the raft, representing proposed v2state.
    pub ss: SoftState,
    /// The index of the last entry to have been applied.
    pub applied: u64,
    /// The progress towards catching up and applying logs.
    pub progress: Option<&'a ProgressTracker>,
}

impl<'a> BaseStatus<'a> {
    /// Gets a copy of the current raft status.
    pub fn new<T: Storage>(raft: &'a Raft<T>) -> BaseStatus<'a> {
        let mut s = BaseStatus {
            id: raft.id,
            ..Default::default()
        };
        s.hs = raft.hard_state();
        s.ss = raft.soft_state();
        s.applied = raft.raft_log.applied;
        if s.ss.raft_state == StateRole::Leader {
            s.progress = Some(raft.prs());
        }
        s
    }

}

impl<S: Storage> From<&Raft<S>> for BaseStatus<'_>{
    fn from(raft: &Raft<S>) -> Self {
        // let progress = Some(raft.prs());
        BaseStatus{
            id: raft.id,
            hs: raft.hard_state(),
            ss: raft.soft_state(),
            applied: raft.raft_log.applied,
            ..Default::default()
        }
    }
}

impl<S: Storage> From<&Raft<S>> for Status<'_>{
    fn from(raft: &Raft<S>) -> Self {
        let mut s = Status {
            base_status: BaseStatus::from(raft),
            config: Default::default(),
            progress: Default::default(),
        };
        if s.base_status.ss.raft_state == StateRole::Leader {
            s.progress = raft.prs.progress.clone();
        }
        s.config = raft.prs.conf.clone();
        s
    }
}
