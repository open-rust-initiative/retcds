// use prost_build::Config;
// fn main() {
//     let base = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
//     let mut config = prost_build::Config::new();
//     let proto_path = format!("{}/proto", base);
//
//     // config.message_attribute(".",
//     //                       "#[derive(protobuf::Message )]");
//     config.compile_protos(&[&format!("{}/snappb.proto", proto_path)], &[format!("{}/include", base), format!("{}/proto", base)]).unwrap();
// }

use protobuf_build::Builder;

fn main() {
    let base = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    Builder::new()
        .search_dir_for_protos(&format!("{}/proto", base))
        .includes(&[format!("{}/include", base), format!("{}/proto", base)])
        .include_google_protos()
        .generate()
}