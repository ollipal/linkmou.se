use std::sync::{Arc, Mutex};
use std::env;

use serde::{Serialize, Deserialize};

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DesktopInfo{
    linkmouseVersion: String,
    osName: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct BrowserInfo{
    linkmouseVersion: String,
    osName: String,
    browserName: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct BrowserSettings{
    mouseSpeed: f64,
    scrollSpeed: f64,
    scrollReversed: bool,
}

lazy_static! {
    pub static ref DESKTOP_INFO: Arc<Mutex<DesktopInfo>> = Arc::new(Mutex::new(
        DesktopInfo {
            linkmouseVersion: "0.0.1".to_string(),
            osName: env::consts::OS.to_string(),
        }
    ));
    pub static ref BROWSER_INFO: Arc<Mutex<BrowserInfo>> = Arc::new(Mutex::new(
        BrowserInfo {
            linkmouseVersion: "unknown".to_string(),
            osName: "unknown".to_string(),
            browserName: "unknown".to_string(),
        }
    ));
    pub static ref BROWSER_SETTINGS: Arc<Mutex<BrowserSettings>> = Arc::new(Mutex::new(
        BrowserSettings {
            mouseSpeed: 1.00,
            scrollSpeed: 1.00,
            scrollReversed: false,
        }
    ));
}