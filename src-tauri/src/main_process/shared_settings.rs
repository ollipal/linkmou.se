use std::sync::{Arc, Mutex};
use std::env;

use serde::{Serialize, Deserialize};

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct DesktopInfo{
    linkmouseVersion: String,
    os: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct BrowserInfo{
    linkmouseVersion: String,
    os: String,
    browserName: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct BrowserSettings{
    mouseSpeed: f64,
    scrollSpeed: f64,
    scrollReversed: bool,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct SharedState {
    desktopInfo: DesktopInfo,
    browserInfo: BrowserInfo,
    browserSettings: BrowserSettings,
}

lazy_static! {
    pub static ref SHARED_STATE: Arc<Mutex<SharedState>> = Arc::new(Mutex::new(
        SharedState {
            desktopInfo: DesktopInfo {
                linkmouseVersion: "0.0.1".to_string(),
                os: env::consts::OS.to_string(),
            },
            browserInfo: BrowserInfo {
                linkmouseVersion: "unknown".to_string(),
                os: "unknown".to_string(),
                browserName: "unknown".to_string(),
            },
            browserSettings: BrowserSettings {
                mouseSpeed: 1.00,
                scrollSpeed: 1.00,
                scrollReversed: false,
            },
        }
    ));
}