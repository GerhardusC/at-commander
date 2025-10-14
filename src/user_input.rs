use std::{
    error::Error,
    io::{stdin, ErrorKind},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle}, time::Duration,
};

use serialport::SerialPort;

use crate::{args::Args, event_loop::{Event, EventLoop, WifiEvent }, utils::{parse_bytes, wait_for_msg_on_buffer}};


pub fn user_input_task(
    port: &Box<dyn SerialPort>,
    event_loop: &EventLoop,
    read_buffer: Arc<Mutex<String>>,
    args: Args,
) -> Result<JoinHandle<Result<(), ErrorKind>>, Box<dyn Error>> {
    let mut port_cp = port.try_clone()?;
    let event_sender = event_loop.sender.clone();
    let read_buffer_cp = read_buffer.clone();
    let jh = thread::spawn(move || -> Result<(), ErrorKind> {
        loop {
            let mut input = String::new();
            let _bytes_read_to_input = stdin()
                .read_line(&mut input)
                .map_err(|_| ErrorKind::Other)?;

            let mut trimmed_input = input.trim().to_owned();

            // Send raw bytes if string ends with ~
            let payload = if trimmed_input.ends_with("~") {
                trimmed_input.pop();
                let bytes_vec = parse_bytes(&trimmed_input, args.radix_input_buffer)
                    .map_err(|_| ErrorKind::Other)?;
                bytes_vec
            // Send MQTT message
            } else if trimmed_input == "configure" {
                event_sender.send(Event::new(WifiEvent::Configure, "".to_owned()));
                continue;
            } else if trimmed_input.starts_with("start") {
                let addr: Vec<String> = trimmed_input.split(":").map(|x| x.to_owned()).collect();

                event_sender.send(Event::new(
                    WifiEvent::PublishConnectRequest,
                    addr.get(1).unwrap_or(&"243".to_owned()).to_owned(),
                ));
                continue;
            } else if trimmed_input == "reset" {
                event_sender.send(Event::new(WifiEvent::Reset, trimmed_input));
                continue;
            } else if trimmed_input.starts_with("con") {
                event_sender.send(Event::new(WifiEvent::ConnAck, trimmed_input));
                continue;
            } else if trimmed_input.starts_with("msg") {
                event_sender.send(Event::new(
                    WifiEvent::Publish,
                    trimmed_input
                        .split_at_checked(3)
                        .unwrap_or(("", ""))
                        .1
                        .to_owned(),
                ));
                continue;
            } else if trimmed_input == "close" {
                event_sender.send(Event::new(WifiEvent::Close, trimmed_input));
                continue;
                // TODO: Either extract into event or own func.
            } else if trimmed_input.starts_with("full") {
                // NOTE: SHAPE:
                // full:addr:topic:msg
                let args: Vec<String> = trimmed_input.split(":").map(|x| x.to_owned()).collect();
                let default_addr = String::from("243");
                let default_topic = String::from("/home");
                let default_message = String::from("heLLOAS");

                let addr = args.get(1).unwrap_or(&default_addr);
                let topic = args.get(2).unwrap_or(&default_topic);
                let message = args.get(3).unwrap_or(&default_message);

                event_sender.send(Event::new(
                    WifiEvent::PublishConnectRequest,
                    addr.to_owned(),
                ));

                wait_for_msg_on_buffer(
                    "CONNECT",
                    read_buffer_cp.clone(),
                    event_sender.clone(),
                    Event::new(WifiEvent::ConnAck, "".to_owned()),
                );

                wait_for_msg_on_buffer(
                    "SEND OK",
                    read_buffer_cp.clone(),
                    event_sender.clone(),
                    Event::new(WifiEvent::Publish, format!("msg:{}:{}", topic, message)),
                );

                wait_for_msg_on_buffer(
                    "SEND OK",
                    read_buffer_cp.clone(),
                    event_sender.clone(),
                    Event::new(WifiEvent::Close, "".to_owned()),
                );
                continue;
            } else {
                (trimmed_input.to_owned() + "\r\n").as_bytes().to_vec()
            };

            println!("Sending {} bytes", payload.len());

            let res = port_cp.write(payload.as_slice());
            port_cp.flush().map_err(|_| ErrorKind::Other)?;

            match res {
                Ok(x) => {
                    println!("Bytes written: {x}");
                }
                Err(e) => {
                    println!("{e}");
                    break;
                }
            }
        }
        Ok(())
    });
    Ok(jh)
}

