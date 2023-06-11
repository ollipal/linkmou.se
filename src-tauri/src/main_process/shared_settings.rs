use std::sync::{Arc, Mutex};
use std::env;

use serde::{Serialize, Deserialize};

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DesktopInfo{
    pub linkmouseVersion: String,
    pub osName: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct BrowserInfo{
    pub linkmouseVersion: String,
    pub osName: String,
    pub browserName: String,
    pub engineName: String,
}

#[allow(non_snake_case)]
#[derive(Serialize, Deserialize, Debug)]
pub struct BrowserSettings{
    pub mouseSpeed: f64,
    pub mouseAcceleration: bool,
    pub scrollSpeed: f64,
    pub scrollReversed: bool,
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
            engineName: "unknown".to_string(),
        }
    ));
    pub static ref BROWSER_SETTINGS: Arc<Mutex<BrowserSettings>> = Arc::new(Mutex::new(
        BrowserSettings {
            mouseSpeed: 1.00,
            mouseAcceleration: true,
            scrollSpeed: 1.00,
            scrollReversed: false,
        }
    ));
}