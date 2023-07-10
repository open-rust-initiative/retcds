// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

// We use `default` method a lot to be support prost and rust-protobuf at the
// same time. And reassignment can be optimized by compiler.
#![allow(clippy::field_reassign_with_default)]

mod confchange;
mod confstate;

pub use crate::confchange::{
    new_conf_change_single, parse_conf_change, stringify_conf_change, ConfChangeI,
};
pub use crate::confstate::conf_state_eq;
pub use crate::protos::eraftpb;

#[allow(dead_code)]
#[allow(unknown_lints)]
#[allow(clippy::all)]
#[allow(renamed_and_removed_lints)]
#[allow(bare_trait_objects)]
mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

    use protobuf::Message;
    use serde::{Serialize, Serializer};
    use serde::ser::SerializeStruct;
    use crate::eraftpb::{ConfState, SnapshotMetadata};
    use self::eraftpb::Snapshot;


    impl Serialize for ConfState{
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
            let mut s = serializer.serialize_struct("ConfState", 7)?;
            s.serialize_field("voters", &self.get_voters())?;
            s.serialize_field("learners", &self.get_learners())?;
            s.serialize_field("voters_outgoing", &self.get_voters_outgoing())?;
            s.serialize_field("learners_next", &self.get_learners_next())?;
            s.serialize_field("auto_leave", &self.get_auto_leave())?;
            // s.serialize_field("unknown_fields", &self.get_unknown_fields())?;
            // s.serialize_field("cached_size", &self.get_cached_size())?;
            s.end()
        }
    }

    impl Serialize for SnapshotMetadata{
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
            let mut s =serializer.serialize_struct("SnapshotMetadata", 5)?;
            s.serialize_field("conf_state", &self.get_conf_state())?;
            s.serialize_field("index", &self.get_index())?;
            s.serialize_field("term", &self.get_term())?;
            // s.serialize_field("unknown_fields", &self.get_unknown_fields())?;
            // s.serialize_field("cached_size", &self.get_cached_size())?;
            s.end()
        }
    }

    impl Serialize for Snapshot{
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
            let mut s = serializer.serialize_struct("Snapshot", 4)?;
            s.serialize_field("metadata", &self.get_metadata())?;
            s.serialize_field("data", &self.get_data())?;
            // s.serialize_field("unknown_fields", &self.get_unknown_fields())?;
            // s.serialize_field("cached_size", &self.get_cached_size())?;
            s.end()
        }
    }
    impl Snapshot {
        /// For a given snapshot, determine if it's empty or not.
        pub fn is_empty(&self) -> bool {
            self.get_metadata().index == 0
        }
    }

}

pub mod prelude {
    pub use crate::eraftpb::{
        ConfChange, ConfChangeSingle, ConfChangeTransition, ConfChangeType, ConfChangeV2,
        ConfState, Entry, EntryType, HardState, Message, MessageType, Snapshot, SnapshotMetadata,
    };
}

pub mod util {
    use crate::eraftpb::ConfState;

    impl<Iter1, Iter2> From<(Iter1, Iter2)> for ConfState
    where
        Iter1: IntoIterator<Item = u64>,
        Iter2: IntoIterator<Item = u64>,
    {
        fn from((voters, learners): (Iter1, Iter2)) -> Self {
            let mut conf_state = ConfState::default();
            conf_state.mut_voters().extend(voters.into_iter());
            conf_state.mut_learners().extend(learners.into_iter());
            conf_state
        }
    }
}
