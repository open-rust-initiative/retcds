//
//
// pub use crate::snap::Snapshot;
//
//
// #[allow(dead_code)]
// #[allow(unknown_lints)]
// #[allow(clippy::all)]
// #[allow(renamed_and_removed_lints)]
// #[allow(bare_trait_objects)]
// pub mod snap{
//     include!(concat!(env!("OUT_DIR"), "/snap.rs"));
// }

pub use crate::protos::snappb;

#[allow(dead_code)]
#[allow(unknown_lints)]
#[allow(clippy::all)]
#[allow(renamed_and_removed_lints)]
#[allow(bare_trait_objects)]
mod protos {
    include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

}


