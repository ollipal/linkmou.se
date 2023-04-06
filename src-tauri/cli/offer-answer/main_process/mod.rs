mod datachannel;
use webrtc::data_channel::{data_channel_message::DataChannelMessage, OnMessageHdlrFn};

use crate::main_process::datachannel::new_main;

pub async fn main_process() {
    let on_message: OnMessageHdlrFn  = Box::new(move |msg: DataChannelMessage| {
        
        //println!("Message from DataChannel '{d_label}': '{msg_str}'");
        Box::pin(async move {})
    });

    new_main(on_message).await;
}