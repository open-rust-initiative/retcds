use std::time;
use crc::{Crc, CRC_32_ISCSI};
use crate::etcdserver::api::rafthttp::error::Error;
use crate::etcdserver::api::rafthttp::snap;
use bincode::{serialize, deserialize};
// use raft::eraftpb::Snapshot;

pub type Result<T> = std::result::Result<T, Error>;
const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);

const snap_suffix: &str =".snap";


pub struct SnapShotter{
    dir: Box<str>,
    logger: slog::Logger
}

impl SnapShotter{
    fn new(dir: Box<str>, logger: slog::Logger) -> Self {
        SnapShotter{
            dir: dir,
            logger:logger
        }
    }

    // fn save_snap(&self, snap:&Snapshot){
    //     if snap.is_empty(){
    //         panic!("empty snapshot");
    //     }
    //
    //     return self.
    // }
    fn save(&self, snap:&Snapshot) -> Result<()>{
        let mut start = time::SystemTime::now();
        let mut fname = format!("{}-{}{}", snap.get_metadata().get_index(), snap.get_metadata().get_term(), snap_suffix);

        let vec = serialize(snap).unwrap();
        let mut digest = CASTAGNOLI.digest();

        let crc = digest.update(&vec);

        return Ok(())


    }

}