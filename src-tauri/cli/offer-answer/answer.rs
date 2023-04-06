#[macro_use]
extern crate lazy_static;

mod datachannel;
use crate::datachannel::new_main;

#[tokio::main]
async fn main() {
    new_main().await;
}
