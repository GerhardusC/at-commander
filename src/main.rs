use std::{
    error::Error, io::{stdin, ErrorKind}, thread::{self}, time::Duration
};

use at_commander::{Event, EventLoop};
use clap::{command, Parser};

/// Simple program to communicate AT commands with the ESP-01 module.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port
    #[arg(short, long, default_value_t = String::from("/dev/ttyUSB0"))]
    port: String,

    /// Baud rate
    #[arg(short, long, default_value_t = 115_200)]
    baud_rate: u32,

    /// By ending an input string with the "~" character, you may specify to send a buffer
    /// instead of an ASCII string. This argument determines the base in which your buffer will
    /// be interpreted. By default it is 16, i.e. your buffers need to be HEX values split by
    /// spaces. E.G. "0F F0 30 40 5C~" etc. 
    #[arg(short, long, default_value_t = 16)]
    radix_input_buffer: u8,
}

fn parse_bytes(input: &str, radix: u8) -> Result<Vec<u8>, String> {
    input
        .split_whitespace()
        .map(|token| u8::from_str_radix(token, radix.into())
        .map_err(|_| format!("Invalid hex: {}", token)))
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let event_loop = EventLoop::new();

    let mut port = serialport::new(&args.port, args.baud_rate)
        .timeout(Duration::from_millis(100))
        .open()?;

    let mut port_cp1 = port.try_clone()?;
    let mut port_cp2 = port.try_clone()?;

    // READING TASK
    let tr1 = thread::spawn(move || {
        loop {
            let mut buffer: [u8; 1] = [0; 1];
            match port_cp1.read(&mut buffer) {
                Ok(bytes) => {
                    if bytes == 1 {
                        print!("{}", String::from_utf8_lossy(&buffer));
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => (),
                Err(e) => {
                    eprintln!("{:?}", e);
                    break;
                },
            }
        }
    });

    // USER INPUT TASK
    let tr2_sender = event_loop.sender.clone();
    let tr2 = thread::spawn(move || -> Result<(), ErrorKind> {
        loop {
            let mut input = String::new();
            let _bytes_read_to_input = stdin().read_line(&mut input).map_err(|_| ErrorKind::Other)?;

            let mut trimmed_input = input.trim().to_owned();

            // Send raw bytes if string ends with ~
            let payload = if trimmed_input.ends_with("~") {
                trimmed_input.pop();
                let bytes_vec = parse_bytes(&trimmed_input, args.radix_input_buffer).map_err(|_| ErrorKind::Other)?;
                bytes_vec
            // Send MQTT message
            } else if trimmed_input.starts_with("con") {
                // Client name always client 1.
                // Byte #	Value	Field	Description
                // 1	10	Fixed header	CONNECT packet, flags=0
                // 2	13	Remaining Length	19 bytes
                // 3-4	00 04	Protocol Name Length	4 bytes
                // 5-8	4D 51 54 54	Protocol Name	"MQTT"
                // 9	04	Protocol Level	MQTT 3.1.1
                // 10	02	Connect Flags	CleanSession=1
                // 11-12	00 3C	Keep Alive	60 seconds
                // 13-14	00 07	Client ID length	7 bytes
                // 15-21	63 6C 69 65 6E 74 31	Client ID	"client1"
                let msg = "\x10\x13\x00\x04MQTT\x04\x02\x00\x3C\x00\x07client1".as_bytes().to_vec();
                msg
            } else if trimmed_input.starts_with("msg") {
                let args: Vec<String> = trimmed_input.split(":").map(|x| x.to_owned()).collect();

                let topic = match args.get(1) {
                    Some(x) => x.to_owned(),
                    None => "/home".to_owned(),
                };

                let message = match args.get(2) {
                    Some(x) => x.to_owned(),
                    None => "HEY".to_owned(),
                };

                let topic_len = topic.len();
                let topic_and_message_len = topic_len + message.len();

                // Byte #	Value	Field	Description
                // 1	30	Fixed header	PUBLISH packet, QoS=0, DUP=0, Retain=0
                // 2	12	Remaining Length	18 bytes (variable header + payload)
                // 3-4	00 0B	Topic Name Length	11 bytes
                // 5-15	2F 74 65 73 74 2F 74 6F 70 69 63	Topic Name = "/test/topic"	
                // 16-20	68 65 6C 6C 6F	Payload = "hello"
                let mut buff: Vec<u8> = vec![
                    0x30,
                    topic_and_message_len.try_into().unwrap_or(0xFF),
                    if topic_len > 0xFF { (topic_len - 0xFF).try_into().unwrap_or(255) } else { 0x00 },
                    topic_len.try_into().unwrap_or(0xFF),
                ];

                let mut topic_msg_u8 = (topic + &message).into_bytes();

                buff.append(&mut topic_msg_u8);
                buff
            } else if trimmed_input == "test" {
                tr2_sender.send(
                    Event::new("connect".to_owned(), "asdasd".to_owned())
                );
                "\r\n".as_bytes().to_vec()

            } else {
                (trimmed_input.to_owned() + "\r\n").as_bytes().to_vec()
            };

            println!("Sending {} bytes", payload.len());

            let res = port.write(payload.as_slice());
            port.flush().map_err(|_| ErrorKind::Other)?;

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

    let connect_responsed_sender = event_loop.sender.clone();
    event_loop.on("connect".to_owned(), move |e| {
        // port_cp2.write("".as_bytes());
        println!("CONNECT EVENT RECEIVED");
        connect_responsed_sender.send(
            Event::new("connected".to_owned(), "".to_owned())
        );
    });

    event_loop.on("connected".to_owned(), |e| {
        println!("CONNECTED EVENT RECEIVED");
    });

    event_loop.start();

    tr1.join().map_err(|_e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("Something went wrong."))
    })?;
    let res = tr2.join().map_err(|_e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("Something went wrong."))
    })?;
    match res {
        Ok(_) => {
            println!("Returned without errors");
        },
        Err(_e) => {
            println!("Something went wrong somewhere, if your code were better, we would know where");
        },
    }

    Ok(())
}
