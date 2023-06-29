use super::*;
use slog::{Drain, Level, Logger, o};
use std::sync::Arc;
use raft_proto::eraftpb::{Message, MessageType};

#[test]
fn test_network() {
    let drain = slog::Discard.fuse();
    let logger = slog::Logger::root(drain, o!());

    let nodes: Vec<Option<Interface>> = (1..=3).map(|_| None).collect();
    let mut network = Network::new(nodes, &logger);

    // Send a message from node 1 to node 2.
    let message = Message {
        from: 1,
        to: 2,
        msg_type: MessageType::MsgAppend,
        ..Default::default()
    };
    network.filter_and_send(vec![message]);

    // Ensure the message is delivered and persisted.
    assert_eq!(network.peers.get(&1).unwrap().raft_log.last_index(), 1);
    assert_eq!(network.peers.get(&2).unwrap().raft_log.last_index(), 1);

    // Cut communication between node 1 and 2.
    network.cut(1, 2);

    // Send another message from node 1 to node 2.
    let message = Message {
        from: 1,
        to: 2,
        msg_type: MessageType::MsgAppend,
        ..Default::default()
    };
    network.filter_and_send(vec![message]);

    // Ensure the message is not delivered or persisted.
    assert_eq!(network.peers.get(&1).unwrap().raft_log.last_index(), 1);
    assert_eq!(network.peers.get(&2).unwrap().raft_log.last_index(), 0);

    // Recover the network.
    network.recover();

    // Send another message from node 1 to node 2.
    let message = Message {
        from: 1,
        to: 2,
        msg_type: MessageType::MsgAppend,
        ..Default::default()
    };
    network.filter_and_send(vec![message]);

    // Ensure the message is delivered and persisted.
    assert_eq!(network.peers.get(&1).unwrap().raft_log.last_index(), 2);
    assert_eq!(network.peers.get(&2).unwrap().raft_log.last_index(), 2);

    // Ignore all messages of type `MsgAppend`.
    network.ignore(MessageType::MsgAppend);

    // Send another message from node 1 to node 2.
    let message = Message {
        from: 1,
        to: 2,
        msg_type: MessageType::MsgAppend,
        ..Default::default()
    };
    network.filter_and_send(vec![message]);

    // Ensure the message is not delivered or persisted.
    assert_eq!(network.peers.get(&1).unwrap().raft_log.last_index(), 2);
    assert_eq!(network.peers.get(&2).unwrap().raft_log.last_index(), 2);
}