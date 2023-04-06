#[macro_use]
extern crate lazy_static;

mod main_process;
use crate::main_process::main_process;

#[tokio::main]
async fn main() {
    main_process().await;
}
