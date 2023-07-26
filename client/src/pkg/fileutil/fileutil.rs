use std::{fs, io};
use std::fs::{create_dir, File, metadata, Permissions, ReadDir, remove_file};
use std::io::{Error, ErrorKind, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use slog::{error, info, warn};
use crate::pkg::fileutil::read_dir::read_dir;
use crate::pkg::tlsutil::default_logger;

// PrivateFileMode grants owner to read/write a file.
const  PrivateFileMode: u32 = 0o600;

const 	PrivateDirMode:u32= 0o777;



// IsDirWriteable checks if dir is writable by writing and removing a file
// to dir. It returns nil if dir is writable.
pub fn is_dir_writeable(dir : &str) -> std::io::Result<()>{
    let mut path = PathBuf::from(dir);
    path.push(".touch");
    let mut file = File::create(&path)?;
    file.write_all(b"test")?;
    remove_file(&path)?;
    Ok(())
}

// TouchDirAll is similar to os.MkdirAll. It creates directories with 0700 permission if any directory
// does not exists. TouchDirAll also ensures the given directory is writable.
pub fn torch_dir_all(dir : &str) -> std::io::Result<()>{
    // If path is already a directory, MkdirAll does nothing and returns nil, so,
    // first check if dir exist with an expected permission mode.
    if exists(dir) {
        let result = check_dir_permission(dir, PrivateDirMode);
        if result.is_err() {
            info!(default_logger(),"check file permission");
        }
    }
        else {
            let create_dir = fs::create_dir_all(dir);
            if create_dir.is_err(){
                error!(default_logger(),"create dir error");
                return Err(Error::new(ErrorKind::Other, format!("create dir error {},reason => {}", dir, create_dir.err().unwrap())));
            }
            let perm =fs::set_permissions(dir, Permissions::from_mode(PrivateDirMode));
            if perm.is_err() {
                error!(default_logger(),"set permission error");
                return Err(Error::new(ErrorKind::Other, format!("set permission error {}", dir)));
            }

        }

    return is_dir_writeable(dir);
}

// Exist returns true if a file or directory exists.
pub fn exists(name: &str) -> bool {
    metadata(name).is_ok()
}

// DirEmpty returns true if a directory empty and can access.
pub fn dir_empty(dir : &str) -> bool{
    let result = read_dir(dir.to_string(), vec![]);
    if result.is_err(){
        return false;
    }
    let names = result.unwrap();
    return names.len() == 0;
}

// ZeroToEnd zeros a file starting from SEEK_CUR to its SEEK_END. May temporarily
// shorten the length of the file.
pub fn zero_to_end(f: &mut File) -> io::Result<()> {
    let off = f.seek(SeekFrom::Current(0))?;
    let lenf = f.seek(SeekFrom::End(0))?;

    // info!(default_logger(),"{}",off);
    // info!(default_logger(),"{}",lenf);
    f.set_len(off)?;


    if let Err(err) = nix::fcntl::fallocate(f.as_raw_fd(), nix::fcntl::FallocateFlags::FALLOC_FL_ZERO_RANGE, off as i64, (lenf - off) as i64) {
        return Err(io::Error::from(err));
    }

    f.seek(SeekFrom::Start(off))?;

    Ok(())
}

pub fn create_dir_all(dir : &str) -> std::io::Result<()>{
    let result = torch_dir_all(dir);
    if result.is_ok(){
        let mut names = Vec::new();
        let read_dir = read_dir(dir.to_string(), vec![]);
        if read_dir.is_err() {
            return Err(Error::new(ErrorKind::Other, format!("err =>{}", read_dir.err().unwrap())));
        }
        names = read_dir.unwrap();
        if names.len() != 0{
            return Err(Error::new(ErrorKind::Other, format!("directory {} not empty", dir)));
        }
    }
    else  {
        return Err(Error::new(ErrorKind::Other, format!("create error {} , reason =>{}", dir, result.err().unwrap())));
    };
    return Ok(());
}

// CheckDirPermission checks permission on an existing dir.
// Returns error if dir is empty or exist with a different permission than specified.
pub fn check_dir_permission(dir : &str,perm :u32) -> std::io::Result<()>{

    if !exists(dir){
        return  Err(Error::new(ErrorKind::Other, format!("directory {} empty, cannot check permission", dir)));
    };
    let result = fs::metadata(dir);
    if result.is_err(){
        return Err(Error::new(ErrorKind::Other, result.err().unwrap()));
    };

    let mode = result.unwrap().mode();
    if mode != perm{
        return Err(Error::new(ErrorKind::Other, format!("directory {} exist with permission {:o}, need {:o}", dir, mode, perm)));
    };
    return Ok(());
}

// RemoveMatchFile deletes file if matchFunc is true on an existing dir
// Returns error if the dir does not exist or remove file fail
pub fn remove_match_file(input_dir :&str, match_func : fn(&str) -> bool) -> std::io::Result<()>{
    if !exists(input_dir){
        return Err(Error::new(ErrorKind::Other, format!("directory {} does not exist", input_dir)));
    };

    // warn!(default_logger(),"{}",input_dir);

    let mut dir = read_dir(input_dir.to_string(), vec![]).expect("read dir error");
    for entry in dir.iter().next() {
        let path_str = entry.as_str();
        if match_func(path_str){
            let file_dir =input_dir.clone().to_string() + "/" + path_str;
            // warn!(default_logger(),"{}",file_dir.clone());
            let result = remove_file(file_dir.clone());
            if result.is_err(){
                return Err(Error::new(ErrorKind::Other, format!("remove file {} error,reason =>{}", file_dir, result.err().unwrap())));
            }
        }

    }
    // warn!(default_logger(),"{}",dir.len());
    // while let entry = dir.iter().next() {
    //     let path_str = entry.as_str();
    //     // warn!(default_logger(),"{}",path_str);
    //     if match_func(path_str){
    //         let file_dir =input_dir.clone().to_string() + "/" + path_str;
    //         // warn!(default_logger(),"{}",file_dir.clone());
    //         let result = remove_file(file_dir.clone());
    //         if result.is_err(){
    //             return Err(Error::new(ErrorKind::Other, format!("remove file {} error,reason =>{}", file_dir, result.err().unwrap())));
    //         }
    //     }
    // }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::env::temp_dir;
    use std::{env, fs};
    use std::fs::{create_dir, File, read, remove_dir, remove_dir_all};
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use nix::sys::socket::SockaddrLike;
    use crate::pkg::fileutil::fileutil::{create_dir_all, default_logger};
    use slog::{info, warn};
    use super::*;

    #[test]
    fn test_is_dir_writeable() {
        let mut temp_dir = temp_dir();
        temp_dir.push("test_writeable");
        create_dir_all(temp_dir.clone().to_str().unwrap()).expect("create dir error");
        let result = super::is_dir_writeable(temp_dir.clone().to_str().unwrap());
        assert!(result.is_ok());
        let mut temp_perm = fs::metadata(temp_dir.clone().to_str().unwrap()).unwrap().permissions();
        temp_perm.set_mode(0o444);
        let mode = fs::set_permissions(temp_dir.clone(), temp_perm);
        let write = super::is_dir_writeable(temp_dir.clone().to_str().unwrap());
        let username = env::var_os("USER").and_then(|s| s.into_string().ok());
        // warn!(default_logger(),"{}",username.unwrap());
        if username.unwrap() == "root" || env::consts::OS == "windows" {
            warn!(default_logger(),"running as a superuser or in windows");
            return;
        }
        assert!(write.is_err());
        remove_dir(temp_dir.clone()).expect("remove dir error");
    }


    #[test]
    fn test_create_dir_all() {
        let mut temp_dir = temp_dir();
        temp_dir.push("test_dir");
        // 判断目录是否存在
        if !temp_dir.exists() {
            create_dir_all(temp_dir.clone().to_str().unwrap()).expect("create dir error");
        }

        let mut file_dir = temp_dir.clone();
        file_dir.push("test.txt");
        info!(default_logger(),"{}",file_dir.to_str().unwrap());

        // 判断文件是否存在
        if !temp_dir.exists() {
            let mut file = File::create(file_dir.clone()).expect("error");
            assert!(file.write(b"test").is_ok());
        }
        remove_dir(temp_dir.clone()).expect("remove dir error");
    }

    #[test]
    fn test_exist() {
        let mut temp_dir = temp_dir();
        temp_dir.push("test_exist");
        if !exists(temp_dir.clone().to_str().unwrap()) {
            create_dir_all(temp_dir.clone().to_str().unwrap()).expect("create dir error");
        }
        let result = exists(temp_dir.clone().to_str().unwrap());
        assert!(result);

        let mut file_dir = temp_dir.clone();
        file_dir.push("fileutil");

        assert!(!exists(file_dir.clone().to_str().unwrap()));
        // 判断文件是否存在
        if !exists(file_dir.clone().to_str().unwrap()) {
            let mut file = File::create(file_dir.clone()).expect("error");
            assert!(file.write(b"test").is_ok());
            assert!(exists(file_dir.clone().to_str().unwrap()));
            remove_file(file_dir.clone()).expect("error remove file");
            assert!(!exists(file_dir.clone().to_str().unwrap()))
        }
        remove_dir(temp_dir.clone()).expect("remove dir error");
    }

    #[test]
    fn test_dir_empty() {
        let mut temp_dir = temp_dir();
        temp_dir.push("test_dir_empty");
        if !exists(temp_dir.clone().to_str().unwrap()) {
            create_dir_all(temp_dir.clone().to_str().unwrap()).expect("create dir error");
        }
        assert!(dir_empty(temp_dir.clone().to_str().unwrap()));
        let mut file_dir = temp_dir.clone();
        file_dir.push("fileutil");

        if !exists(file_dir.clone().to_str().unwrap()) {
            let mut file = File::create(file_dir.clone()).expect("error");
            assert!(file.write(b"test").is_ok());
            assert!(!dir_empty(temp_dir.clone().to_str().unwrap()));
            remove_file(file_dir.clone()).expect("error remove file");
            assert!(dir_empty(temp_dir.clone().to_str().unwrap()))
        }
        remove_dir(temp_dir.clone()).expect("remove dir error");
    }

    #[test]
    fn test_zero_to_end() {
        let mut file_dir = temp_dir();
        file_dir.push("fileutil");
        if !exists(file_dir.clone().to_str().unwrap()) {
            let mut file = File::create(file_dir.clone()).expect("error");

            // zero_to_end(&mut file).err().unwrap().to_string();
            info!(default_logger(),"{}",zero_to_end(&mut file).err().unwrap().to_string());
            assert!(zero_to_end(&mut file).is_err());
            let mut b: Vec<u8> = Vec::with_capacity(1024);
            for _ in 0..1024 {
                b.push(12);
            }
            file.write(&b).expect("write error");
            let seek = SeekFrom::Start(512);
            file.seek(seek).expect("seek error");
            assert!(zero_to_end(&mut file).is_ok());

            let i = file.seek(SeekFrom::Current(0)).expect("seek error");
            assert_eq!(i, 512);
            let mut b2: Vec<u8> = Vec::with_capacity(512);
            b2 = read(file_dir.clone()).expect("read error");
            for i in 0..512 {
                assert_eq!(b2[i], 12);
            }
            remove_file(file_dir);
        }
    }

    #[test]
    fn test_dir_permission() {
        let mut temp_dir = temp_dir();
        temp_dir.push("test_dir_permission");
        if !exists(temp_dir.clone().to_str().unwrap()) {
            create_dir_all(temp_dir.clone().to_str().unwrap()).expect("create dir error");
        }
        let perm = check_dir_permission(temp_dir.clone().to_str().unwrap(), 0o600);
        warn!(default_logger(),"{}",perm.err().unwrap().to_string());
        // assert!(perm.is_err());
        remove_dir(temp_dir.clone()).expect("remove dir error");
    }

    #[test]
    fn test_remove_match_file() {
        let mut temp_dir = temp_dir();
        temp_dir.push("test_remove_match_file");
        if !exists(temp_dir.clone().to_str().unwrap()) {
            create_dir_all(temp_dir.clone().to_str().unwrap()).expect("create dir error");
        }
        let mut file_dir = temp_dir.clone();
        file_dir.push("test.txt");
        File::create(file_dir.clone()).expect("error").write(b"test").expect("write error");

        let result = remove_match_file(temp_dir.clone().to_str().unwrap(), |s| s.ends_with("txt"));
        // warn!(default_logger(),"{}",result.err().unwrap().to_string());
        assert!(result.is_ok());
        remove_dir_all(temp_dir.clone()).expect("remove dir error");

    }
}