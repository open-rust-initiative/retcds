use std::fs;
use std::fs::{File, metadata, Permissions, remove_file};
use std::io::{ErrorKind, Write,Error};
use std::os::windows::fs::MetadataExt;
use std::path::PathBuf;
use slog::info;
use winapi::shared::minwindef::DWORD;
use winapi::um::fileapi::CreateDirectoryA;
use winapi::um::winnt;
use crate::pkg::tlsutil::default_logger;

// PrivateFileMode grants owner to read/write a file.
const  PrivateFileMode: DWORD = winnt::FILE_GENERIC_WRITE | winnt::FILE_GENERIC_READ;


// IsDirWriteable checks if dir is writable by writing and removing a file
// to dir. It returns nil if dir is writable.
fn is_dir_writeable(dir : &str) -> std::io::Result<()>{
    let mut path = PathBuf::from(dir);
    path.push(".touch");
    let mut file = File::create(&path)?;
    file.write_all(b"")?;
    remove_file(&path)?;
    Ok(())
}

// TouchDirAll is similar to os.MkdirAll. It creates directories with 0700 permission if any directory
// does not exists. TouchDirAll also ensures the given directory is writable.
fn torch_dir_all(dir : &str) -> std::io::Result<()>{
    // If path is already a directory, MkdirAll does nothing and returns nil, so,
    // first check if dir exist with an expected permission mode.
    if exists(dir){
        let result = check_dir_permission(dir, PrivateFileMode);
        if result.is_err(){
            info!(default_logger(),"check file permission");
        }
        else {
            let create_dir = fs::create_dir_all(dir);
            // fs::set_permissions(dir,Permissions::from(PrivateFileMode))
        }
    }
    return Ok(())
}

// Exist returns true if a file or directory exists.
fn exists(name: &str) -> bool {
    metadata(name).is_ok()
}


// CheckDirPermission checks permission on an existing dir.
// Returns error if dir is empty or exist with a different permission than specified.
fn check_dir_permission(dir : &str,pem :DWORD) -> std::io::Result<()>{
    let mut path = PathBuf::from(dir);
    let dir_exists = metadata(&path);

    if !dir_exists.is_ok(){
        let err = format!("directory {} empty, cannot check permission",dir);
        return Err(Error::new(ErrorKind::Other, err));
    };

    let permissions = dir_exists.unwrap().file_attributes();
    if permissions != pem{
        return Err(Error::new(ErrorKind::Other, format!("directory {} exist, but the permission is . The recommended permission is {} to prevent possible unprivileged access to the data",dir,PrivateFileMode.to_string())));
    };
    return Ok(())


}