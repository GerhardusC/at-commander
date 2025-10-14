use std::{
    sync::{Arc, Mutex, mpsc::Sender},
    thread,
    time::Duration,
};

use crate::event_loop::Event;

pub fn parse_bytes(input: &str, radix: u8) -> Result<Vec<u8>, String> {
    input
        .split_whitespace()
        .map(|token| {
            u8::from_str_radix(token, radix.into()).map_err(|_| format!("Invalid hex: {}", token))
        })
        .collect()
}

pub fn wait_for_msg_on_buffer(
    msg: &str,
    read_buffer: Arc<Mutex<String>>,
    event_sender: Sender<Event>,
    event: Event,
) {
    let mut timeout = 0;
    if let Ok(mut read_buffer) = read_buffer.lock() {
        read_buffer.clear();
    }
    loop {
        if let Ok(read_buffer) = read_buffer.lock() {
            if read_buffer.contains(&msg.to_owned()) {
                if let Err(e) = event_sender.send(event) {
                    println!("{e}");
                };
                break;
            }
        }
        if timeout > 1000 {
            break;
        }
        timeout += 1;
        thread::sleep(Duration::from_millis(1));
    }
}

// TODO: Add some tests here
