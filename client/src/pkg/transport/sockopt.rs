
use std::os::unix::io::RawFd;
use std::net::SocketAddr;
use std::os::unix::net::UnixDatagram;
use std::os::unix::io::AsRawFd;
use nix::libc;
use std::net::TcpStream;

type ControlFunc = dyn Fn(&str, &SocketAddr, RawFd) -> Result<(), std::io::Error>;

struct Controls {
    funcs: Vec<Box<ControlFunc>>,
}

impl Controls {
    fn control(&self, network: &str, addr: &SocketAddr, conn: RawFd) -> Result<(), std::io::Error> {
        for f in self.funcs.iter() {
            f(network, addr, conn)?;
        }
        Ok(())
    }
}

pub struct SocketOpts {
    reuse_port: bool,
    reuse_address: bool,
}

impl SocketOpts {
    pub fn empty(&self) -> bool {
        !self.reuse_port && !self.reuse_address
    }
    pub fn get_reuse_port(&self) -> bool {
        self.reuse_port
    }
    pub fn get_reuse_address(&self) -> bool {
        self.reuse_address
    }
}

pub fn get_controls(sopts: &SocketOpts) -> Controls {
    let mut ctls = Controls { funcs: Vec::new() };
    if sopts.reuse_address {
        ctls.funcs.push(Box::new(set_reuse_address));
    }
    if sopts.reuse_port {
        ctls.funcs.push(Box::new(set_reuse_port));
    }
    ctls
}

pub fn set_reuse_address(_network: &str, _addr: &SocketAddr, fd: RawFd) -> Result<(), std::io::Error> {
    let enable = true;
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &enable as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    Ok(())
}

pub fn set_reuse_port(_network: &str, _addr: &SocketAddr, fd: RawFd) -> Result<(), std::io::Error> {
    let enable = true;
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEPORT,
            &enable as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        )
    };
    Ok(())
}