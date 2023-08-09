use std::sync::{Arc, Mutex};
use url::Url;
use crate::etcdserver::api::rafthttp::types::urls::URLs;


#[derive(Clone,Debug)]
pub struct urlPicker{
    base_url_picker : Arc<Mutex<BaseurlPicker>>,
}

impl urlPicker{
    pub fn new_url_picker(urls:URLs) -> urlPicker{
        urlPicker{
            base_url_picker : Arc::new(Mutex::new(BaseurlPicker::new(urls)))
        }
    }

    pub fn get_base_url_picker(&self) -> Arc<Mutex<BaseurlPicker>>{
        self.base_url_picker.clone()
    }
}

#[derive(Debug)]
pub struct BaseurlPicker{
    urls : URLs,
    picked : isize
}

impl BaseurlPicker {
    pub fn new(urls:URLs) -> BaseurlPicker {
        BaseurlPicker{
            urls,
            picked : 0
        }
    }

    pub fn update(&mut self, urls:URLs){
        self.urls = urls;
        self.picked = 0;
    }

    pub fn pick(&mut self) -> Option<Url>{
        return self.urls.get_url(self.picked as usize);
    }

    pub fn unreachable(&mut self, url:Url){
        if url == self.urls.get_url(self.picked as usize).unwrap(){
            self.picked = (self.picked+1)%self.urls.len() as isize;
        }
    }
}

#[cfg(test)]
mod tests{
    use std::collections::HashMap;
    use slog::error;
    use url::Url;
    use crate::etcdserver::api::rafthttp::types::urls::new_urls;
    use crate::etcdserver::api::rafthttp::types::default_logger;
    use crate::etcdserver::api::rafthttp::url_pick::urlPicker;

    #[test]
    fn test_url_picker_pick_twice(){
        let urls = vec!["http://127.0.0.1:2380".to_string(), "http://127.0.0.1:7001".to_string()];

        let URLs = new_urls(urls).unwrap();
        let mut url_picker = urlPicker::new_url_picker(URLs);
        let u = url_picker.base_url_picker.lock().unwrap().pick();
        let mut url_map = HashMap::new();
        url_map.insert(Url::parse("http://127.0.0.1:7001").unwrap(),true);
        url_map.insert(Url::parse("http://127.0.0.1:2380").unwrap(),true);

        assert!(url_map.get(&u.clone().unwrap()).unwrap());

        let uu = url_picker.base_url_picker.lock().unwrap().pick();

        assert_eq!(u,uu);
    }

    #[test]
    fn test_url_picker_update(){
        let urls = vec!["http://127.0.0.1:2380".to_string(), "http://127.0.0.1:7001".to_string()];

        let URLs = new_urls(urls).unwrap();
        let mut url_picker = urlPicker::new_url_picker(URLs);
        let new_url = vec!["http://localhost:2380".to_string(), "http://localhost:7001".to_string()];

        let new_url = new_urls(new_url).unwrap();
        url_picker.base_url_picker.lock().unwrap().update(new_url);
        let u = url_picker.base_url_picker.lock().unwrap().pick();
        let mut url_map = HashMap::new();
        url_map.insert(Url::parse("http://localhost:7001").unwrap(),true);
        url_map.insert(Url::parse("http://localhost:2380").unwrap(),true);
        assert!(url_map.get(&u.clone().unwrap()).unwrap());
    }

    #[test]
    fn test_url_picker_unreachable(){
        let urls = vec!["http://127.0.0.1:2380".to_string(), "http://127.0.0.1:7001".to_string()];

        let URLs = new_urls(urls).unwrap();
        let mut url_picker = urlPicker::new_url_picker(URLs);
        let u = url_picker.base_url_picker.lock().unwrap().pick();
        url_picker.base_url_picker.lock().unwrap().unreachable(u.clone().unwrap());
        let uu = url_picker.base_url_picker.lock().unwrap().pick();
        // assert_eq!(u.clone(),uu.clone())
        assert!((u.unwrap() != uu.unwrap()));
    }
}


