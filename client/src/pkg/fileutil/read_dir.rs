use std::fs;
use std::path::Path;

#[derive(Clone)]
pub struct ReadDirOp {
    ext: String,
}

type ReadDirOption = Box<dyn FnOnce(&mut ReadDirOp)>;

fn with_ext(ext: String) -> ReadDirOption {
    Box::new(move |op| op.ext = ext)
}

impl ReadDirOp {
    fn new() -> Self {
        ReadDirOp {
            ext: String::from(""),
        }
    }

    fn apply_opts(&mut self, opts:Vec<ReadDirOption>) {
        for opt in opts {
            opt(self);
        }
    }
}

pub fn read_dir(d:String,opts: Vec<ReadDirOption>) -> std::io::Result<Vec<String>>{
    let mut op = ReadDirOp::new();
    op.apply_opts(opts);
    let dir = Path::new(&d);
    let mut entries = fs::read_dir(dir)?;

    let mut tss :Vec::<String> = Vec::with_capacity(0);
    let mut names :Vec::<String> = Vec::with_capacity(0);
    for entry in entries.into_iter(){
        names.push(entry.unwrap().file_name().clone().to_str().unwrap().to_string());
    }
    names.sort();

    if op.ext != ""{
        // let mut tss :Vec::<String> = Vec::with_capacity(0);
        for mut name in names.into_iter(){
            if name.ends_with(&op.ext){
                // name = d.clone() + "/" + &name;
                tss.push(name);
            }
        }
        names = tss;
    }
    return Ok(names)
}

#[cfg(test)]
mod tests{
    use std::env::temp_dir;
    use std::fs::{create_dir, File};
    use std::path::PathBuf;
    // use openssl::x509::store::File;
    use slog::{info, warn};
    use super::*;
    use crate::pkg::tlsutil::default_logger;

    #[test]
    fn test_read_dir(){
        let mut temp_dir = temp_dir();
        temp_dir.push("test_read_dir");
        if let Err(metadata) = fs::metadata(temp_dir.clone()) {
            create_dir(temp_dir.to_str().unwrap()).expect("create dir error");
        }
        let mut files = vec!["def", "abc", "xyz", "ghi"];
        for file in files.iter() {
            let string = temp_dir.to_str().unwrap().to_string() + "/" + file;
            let func = wirete_func(string.clone());
        };
        let vec1 = read_dir(temp_dir.to_str().unwrap().to_string(), vec![]).expect("error");
        // warn!(default_logger(),"{}",vec1.len());
        // assert!(vec1.len() == 8);
        let mut result = vec!["abc", "def", "ghi", "xyz","def.wal", "abc.wal", "xyz.wal", "ghi.wal"];
        result.sort();
        assert!(vec1.eq(result.as_slice()));

        let mut files2 = vec!["def.wal", "abc.wal", "xyz.wal", "ghi.wal"];
        for file in files2.iter() {
            let string = temp_dir.to_str().unwrap().to_string() + "/" + file;
            let func = wirete_func(string.clone());
        };
        let vec2 = read_dir(temp_dir.to_str().unwrap().to_string(), vec![with_ext(String::from("wal"))]).expect("error");
        for item in vec2.iter() {
            warn!(default_logger(),"{}",item);
        }
        assert!(vec2.eq(&vec!["abc.wal", "def.wal", "ghi.wal", "xyz.wal"]));

    }

    fn wirete_func(path:String) -> bool{
        let result = File::create(path);
        if result.is_err(){
            warn!(default_logger(),"create file error");
            false
        }
        else {
            true
        };

        true
    }
}