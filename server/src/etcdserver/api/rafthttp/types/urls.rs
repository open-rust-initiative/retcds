use std::io::{Error, ErrorKind};
use std::ops::Add;
use std::str::FromStr;
use slog::warn;
use url::Url;
use crate::etcdserver::api::rafthttp::types::default_logger;

pub struct URLs(Vec<Url>);

impl URLs{
    pub fn String(&self) -> String{
        let mut result = String::new();
        for i in 0..self.0.len(){
            result.push_str(self.0.get(i).expect("REASON").to_string().as_str());
            result.push_str(",");
        }
        return result;
    }
    pub fn len(&self) -> usize{
        self.0.len()
    }

    pub fn less(&self, i:usize,j:usize) -> bool{
        return self.0.get(i).expect("fail get str ").to_string() < self.0.get(j).expect("fail get str").to_string();
    }

    pub fn swap(&mut self, i: usize, j: usize) {
        let temp = self.0[i].to_string();
        self.0[i] = self.0[j].to_string().parse().unwrap();
        self.0[j] = temp.parse().unwrap();
    }

    pub fn string_slice(&self) -> Vec<String>{
        let mut result = Vec::new();
        for i in 0..self.0.len(){
            result.push(self.0.get(i).expect("fail get str").to_string());
        }
        result
    }

    pub fn sort(&mut self){
        self.0.sort()
    }

    pub fn get_url(&self,index:usize) -> Option<Url>{
        return self.0.get(index).cloned();
    }
}

pub fn new_urls(strs:Vec<String>) -> Result<URLs, Error> {
    // warn!(default_logger(),"new_urls len=>{}",strs.len());
    let mut urls = Vec::with_capacity(strs.len());
    if strs.len() == 0{
        return  Err(Error::new(ErrorKind::Other, "no valid URLs given"))
    }

    for s in strs {
        let s = s.trim();
        let u = Url::from_str(s).unwrap();
        let scheme = u.scheme();
        if scheme != "http" && scheme != "https" && scheme != "unix" && scheme != "unixs" {
            warn!(default_logger(),"URL scheme must be http, https, unix, or unixs: {}", s);
            return  Err(Error::new(ErrorKind::Other, "URL scheme must be http, https, unix, or unixs: {}"))
        }
        let addrs = u.socket_addrs(|| None)?;
        if addrs.is_empty() {
            warn!(default_logger(),"URL address does not have the form \"host:port\": {}", s);
            return  Err(Error::new(ErrorKind::Other, "URL address does not have the form \"host:port\": {}"))
        }
        if u.path() != "/" {
            warn!(default_logger(),"URL must not contain a path: {}", s);
            return  Err(Error::new(ErrorKind::Other, "URL must not contain a path"))
        }
        urls.push(u);
    };
    urls.sort();
    Ok(URLs(urls))
}