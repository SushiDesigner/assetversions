use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::{fs, io, time::SystemTime};

#[derive(Serialize, Deserialize, Debug)]
struct Version {
    version: u32,
    date: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Versions {
    assetid: u64,
    versions: Vec<Version>,
}

impl Versions {
    pub fn sort_versions(&mut self) {
        self.versions.sort_by(|a, b| a.version.cmp(&b.version));
    }
}

fn get_asset(
    asset_id: u64,
    version: Option<u32>,
    latest: bool,
) -> Result<serde_json::Value, reqwest::Error> {
    let url = match latest {
        true => format!("https://assetdelivery.roblox.com/v2/asset?id={}", asset_id),
        false => format!(
            "https://assetdelivery.roblox.com/v2/asset?id={}&version={}",
            asset_id,
            version.unwrap()
        ),
    };

    let resp = reqwest::blocking::get(&url)?.json::<serde_json::Value>()?;
    Ok(resp)
}

#[tokio::main]
async fn main() {
    println!("Please enter a an assetid:");

    let mut input_line = String::new();
    io::stdin()
        .read_line(&mut input_line)
        .expect("Failed to read line");
    let asset_id: u64 = input_line.trim().parse().expect("Input not an integer");

    let asset = match get_asset(asset_id, None, true) {
        Ok(asset) => asset,
        Err(e) => panic!("Couldn't get asset!: {}", e),
    };

    let start = SystemTime::now();

    if asset.get("errors").is_some() {
        panic!("Error: {}", asset.get("errors").unwrap());
    }

    if asset.get("locations").unwrap().as_array().unwrap().len() == 0 {
        panic!("Asset not found");
    } else {
        println!("Asset found");
    }

    let stopped = Arc::new(Mutex::new(false));

    let current_version = Arc::new(Mutex::new(1));

    let versions = Arc::new(Mutex::new(Versions {
        assetid: asset_id,
        versions: Vec::new(),
    }));

    let mut handles = Vec::new();

    while *stopped.lock().unwrap() == false {
        let versions = Arc::clone(&versions);

        let current_version = Arc::clone(&current_version);

        let stopped = Arc::clone(&stopped);

        std::thread::sleep(std::time::Duration::from_millis(150));

        handles.push(tokio::task::spawn(async move {
            let current_version_u32 = *current_version.lock().unwrap();

            *current_version.lock().unwrap() += 1;

            println!("Checking for version: {}", current_version_u32);

            let asset = match get_asset(asset_id, Some(current_version_u32), false) {
                Ok(asset) => asset,
                Err(e) => panic!("Couldn't get asset!: {}", e),
            };

            if asset.get("errors").is_some() {
                *stopped.lock().unwrap() = true;

                return;
            }

            let asset_url = asset.get("locations").unwrap()[0]
                .get("location")
                .unwrap()
                .as_str()
                .unwrap();

            let client = reqwest::blocking::Client::new();

            let resp = client.head(asset_url).send().unwrap();

            let date = resp
                .headers()
                .get("last-modified")
                .unwrap()
                .to_str()
                .unwrap();

            println!("Date: {}", date);

            versions.lock().unwrap().versions.push(Version {
                version: current_version_u32,
                date: date.to_string(),
            });

            versions.lock().unwrap().sort_versions();

            fs::write(
                "versions.json",
                serde_json::to_string(&*versions.lock().unwrap()).unwrap(),
            )
            .unwrap();

            let mut output = File::create("versions.txt").unwrap();

            writeln!(output, "Versions of: {}", asset_id).unwrap();

            for version in &versions.lock().unwrap().versions {
                writeln!(
                    output,
                    "Version: {}, Date: {}",
                    version.version, version.date
                )
                .unwrap();
            }
        }));
    }

    futures::future::join_all(handles).await;

    println!("Took {:?} to get", start.elapsed().unwrap());
}
