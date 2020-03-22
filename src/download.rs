use sha2::{Sha256, Digest};
use std::path::Path;
use std::fs;
use flate2::read::GzDecoder;
use std::io::prelude::*;
use log::{warn, info};
//use url::form_urlencoded;
//use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use std::collections::HashMap;

pub fn download(url: &str) -> String {
    let result = Sha256::digest(url.as_bytes());

    let path = Path::new("cache/").join(hex::encode(&result[..]));
    if path.exists() {
        return fs::read_to_string(path).unwrap();
    }

    let response = reqwest::blocking::get(url).unwrap();
    let body = if response.status() == 404 {
        // Sometimes some armor models return 404, we can't panic when that happens
        warn!("Got response code {} for url {}", response.status(), url);
        "".to_string()
    } else {
        let body = response.bytes().unwrap();

        info!("Downloaded {}: {} bytes", url, body.len());
        if url.ends_with(".gz") {
            // Decompress
            let mut d = GzDecoder::new(&body[..]);
            let mut s = String::new();
            d.read_to_string(&mut s).unwrap();
            s
        } else {
            std::str::from_utf8(&body).unwrap().to_string()
        }
    };
    fs::write(path, body.clone()).unwrap();
    body
}


pub fn download_with_params(url: &str, view: &str, params: &str) -> String {
    let to_hash = url.to_string() + view + params;
    let result = Sha256::digest(to_hash.as_bytes());

    let path = Path::new("cache/").join(hex::encode(&result[..]));
    if path.exists() {
        return fs::read_to_string(path).unwrap();
    }

    let client = reqwest::blocking::Client::new();

    let mut raw_params = HashMap::new();
    raw_params.insert("view", view);
    raw_params.insert("params", params);

    let response = client.post(url).form(&raw_params).send().unwrap();
    let body = response.bytes().unwrap();
    info!("Downloaded {} with params: {} bytes", url, body.len());
    let body = if url.ends_with(".gz") {
        // Decompress
        let mut d = GzDecoder::new(&body[..]);
        let mut s = String::new();
        d.read_to_string(&mut s).unwrap();
        s
    } else {
        std::str::from_utf8(&body).unwrap().to_string()
    };
    fs::write(path, body.clone()).unwrap();
    body
}
