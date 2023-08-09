use raft::eraftpb::{Message, MessageType};

pub const pipelineMsg: &'static str = "pipeline";
pub const sendSnap: &'static str = "sendMsgSnap";
pub const streamMsg: &'static str = "streamMsg";
pub const streamAppV2: &'static str = "streamMsgAppV2";


pub fn is_msg_app(m:Message) -> bool{
    return m.get_msg_type() == MessageType::MsgAppend;
}

pub fn is_msg_snap(m:Message) -> bool{
    return m.get_msg_type() == MessageType::MsgSnapshot;
}