use std::ffi::OsString;
use std::io::Error;
use std::fs::{File, OpenOptions};
use std::io::Write;
// 导入了windows的文件系统模块，不适用其他系统
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use winapi::um::winnt;
pub type Result<T> = std::result::Result<T, Error>;


pub async fn write_and_sync_file(filename: &PathBuf, content: &[u8], perm :u32) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .access_mode(perm)
        .open(filename)?;


    file.write_all(content)?;
    file.sync_all()?;
    Ok(())

}