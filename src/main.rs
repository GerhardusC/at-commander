use std::{
    error::Error,
    io::{ErrorKind, stdin},
    sync::{Arc, Mutex, mpsc::Sender},
    thread::{self, JoinHandle, sleep},
    time::Duration,
};

use at_commander::{Event, EventLoop, WifiEvent, WifiState};
use clap::{Parser, command};
use serialport::SerialPort;

pub trait TrackWifiState {
    fn change_to(&self, new_state: WifiState);
    fn get(&self) -> WifiState;
}

impl TrackWifiState for Arc<Mutex<WifiState>> {
    fn change_to(&self, new_state: WifiState) {
        match self.lock() {
            Ok(mut state) => *state = new_state,
            Err(_) => println!("Something went wrong while locking wifi state mux"),
        }
    }
    fn get(&self) -> WifiState {
        match self.lock() {
            Ok(state) => (*state).clone(),
            Err(_) => WifiState::Invalid,
        }
    }
}

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
        .map(|token| {
            u8::from_str_radix(token, radix.into()).map_err(|_| format!("Invalid hex: {}", token))
        })
        .collect()
}

fn wait_for_msg_on_buffer(
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

fn read_port_buffer_task(
    port: &Box<dyn SerialPort>,
    read_buffer: Arc<Mutex<String>>,
) -> Result<JoinHandle<()>, Box<dyn Error>> {
    let mut port = port.try_clone()?;
    let jh = thread::spawn(move || {
        loop {
            let mut buffer: [u8; 1] = [0; 1];
            match port.read(&mut buffer) {
                Ok(bytes) => {
                    if bytes == 1 {
                        let bufstr = String::from_utf8_lossy(&buffer);
                        if let Ok(mut input_buffer) = read_buffer.lock() {
                            input_buffer.push_str(&bufstr);
                        }
                        print!("{}", bufstr);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => (),
                Err(e) => {
                    eprintln!("{:?}", e);
                    break;
                }
            }
        }
    });
    Ok(jh)
}

fn user_input_task(
    state: Arc<Mutex<WifiState>>,
    port: &Box<dyn SerialPort>,
    event_loop: &EventLoop,
    read_buffer: Arc<Mutex<String>>,
    args: Args,
) -> Result<JoinHandle<Result<(), ErrorKind>>, Box<dyn Error>> {
    // TODO: use this state to verify state changes are valid.
    let state = state.clone();
    // USER INPUT TASK
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
                event_sender.send(Event::new(WifiEvent::AckReceived, trimmed_input));
                continue;
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
                    Event::new(WifiEvent::AckReceived, "".to_owned()),
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

fn register_event_handlers(
    event_loop: &EventLoop,
    read_buffer: Arc<Mutex<String>>,
    port: &Box<dyn SerialPort>,
) -> Result<(), Box<dyn Error>> {
    // Register handlers / state transitions
    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::PublishConnectRequest, move |e, state| {
        // Open TCP Stream
        let msg = format!("AT+CIPSTART=\"TCP\",\"192.168.0.{}\",1883\r\n", e.data);
        match port_cp.write(msg.as_bytes()) {
            Ok(bytes_written) => {
                state.change_to(WifiState::WaitingConnectAck);
                println!("Bytes written: {}", bytes_written);
            }
            Err(e) => {
                println!("{e}");
                state.change_to(WifiState::Ready);
            }
        };
        // Clear port read string.
        if let Ok(mut read_buffer) = read_buffer_cp.lock() {
            read_buffer.clear();
        };
        let _ = port_cp.flush();
    });

    let mut port_cp = port.try_clone()?;
    event_loop.on(WifiEvent::Configure, move |e, state| {
        if let Err(e) = port_cp.write("ATE0\r\n".as_bytes()) {
            println!("Failed to write message to wifi device");
        };
    });

    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::ConnAck, move |e, state| {
        if let Ok(mut read_buffer) = read_buffer_cp.lock() {
            read_buffer.clear();
        };
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
        let msg = "\x10\x13\x00\x04\x4D\x51\x54\x54\x04\x02\x00\x3C\x00\x07client1".as_bytes();

        // Tell how many bytes will be sent, don't care about bytes written.
        if let Err(e) = port_cp.write(format!("AT+CIPSEND={}\r\n", msg.len()).as_bytes()) {
            println!("{e}");
            state.change_to(WifiState::Ready);
            return;
        };
        let _ = port_cp.flush();

        // Wait for ack from port off this thread.
        if let Ok(mut port_cp) = port_cp.try_clone() {
            let read_buffer_cp = read_buffer_cp.clone();
            thread::spawn(move || {
                let mut timeout = 0;
                loop {
                    if timeout > 10000 {
                        println!("Timed out on waiting ack from port.");
                        state.change_to(WifiState::Ready);
                        return;
                    }
                    if let Ok(read_buffer) = read_buffer_cp.lock() {
                        if read_buffer.contains("OK") {
                            // Port ready to receive connect request.
                            break;
                        }
                    }

                    timeout += 1;
                    sleep(Duration::from_millis(1));
                }
                // Send bytes
                if let Err(e) = port_cp.write(msg) {
                    println!("Failed to write message to wifi device");
                    state.change_to(WifiState::Ready);
                }
                let _ = port_cp.flush();

                state.change_to(WifiState::Connected);
            });
        }
    });

    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::Publish, move |e, state| {
        if let Ok(mut read_buffer) = read_buffer_cp.lock() {
            read_buffer.clear();
        };
        // Tell how many bytes will be sent
        let args: Vec<String> = e.data.split(":").map(|x| x.to_owned()).collect();

        let topic = match args.get(1) {
            Some(x) => x.to_owned(),
            None => "/test/topic".to_owned(),
        };

        let message = match args.get(2) {
            Some(x) => x.to_owned(),
            None => "hello".to_owned(),
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
            topic_and_message_len.try_into().unwrap_or(0xFF) + 0x02,
            0x00,
            topic_len as u8,
        ];

        let mut topic_msg_u8 = (topic + &message).into_bytes();

        buff.append(&mut topic_msg_u8);

        println!("Sending {:?}", buff);

        if let Err(e) = port_cp.write(format!("AT+CIPSEND={}\r\n", buff.len()).as_bytes()) {
            println!("Failed to write message to wifi device");
            state.change_to(WifiState::Ready);
        };

        let _ = port_cp.flush();

        // Wait for ack from port
        if let Ok(mut port_cp) = port_cp.try_clone() {
            let read_buffer_cp = read_buffer_cp.clone();
            thread::spawn(move || {
                let mut timeout = 0;
                loop {
                    if timeout > 10000 {
                        println!("Timed out on waiting ack from port.");
                        state.change_to(WifiState::Ready);
                        return;
                    }
                    if let Ok(read_buffer) = read_buffer_cp.lock() {
                        if read_buffer.contains("OK") {
                            // TODO: Check what this actually needs to contain
                            // Port ready to receive connect request.
                            break;
                        }
                    }

                    timeout += 1;
                    sleep(Duration::from_millis(1));
                }
                // Send bytes
                if let Err(e) = port_cp.write(&buff) {
                    println!("Failed to write message to wifi device");
                    state.change_to(WifiState::Ready);
                };
                let _ = port_cp.flush();

                state.change_to(WifiState::WaitingPublishAck);
            });
        }
    });

    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::AckReceived, move |e, state| {
        if let Ok(mut read_buffer) = read_buffer_cp.lock() {
            read_buffer.clear();
        };
        // Tell how many bytes will be sent
        port_cp.write("AT+CIPSEND=2\r\n".as_bytes());
        let _ = port_cp.flush();
        // Wait for ack from port
        if let Ok(mut port_cp) = port_cp.try_clone() {
            let read_buffer_cp = read_buffer_cp.clone();
            thread::spawn(move || {
                let mut timeout = 0;
                loop {
                    if timeout > 10000 {
                        println!("Timed out on waiting ack from port.");
                        state.change_to(WifiState::Ready);
                        return;
                    }
                    if let Ok(read_buffer) = read_buffer_cp.lock() {
                        if read_buffer.contains("OK") {
                            // Port ready to receive connect request.
                            break;
                        }
                    }

                    timeout += 1;
                    sleep(Duration::from_millis(1));
                }
                // Send bytes
                port_cp.write(&[0xE0, 0x00]);
                state.change_to(WifiState::Sent);
            });
        }
    });

    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::Close, move |e, state| {
        if let Ok(mut read_buffer) = read_buffer_cp.lock() {
            read_buffer.clear();
        };
        // Tell how many bytes will be sent
        // Wait for ack from port
        // Send bytes
        state.change_to(WifiState::Ready);
    });

    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::Timeout, move |e, state| {
        if let Ok(mut read_buffer) = read_buffer_cp.lock() {
            read_buffer.clear();
        };
        state.change_to(WifiState::Ready);
    });
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let event_loop = EventLoop::new();

    let port = serialport::new(&args.port, args.baud_rate)
        .timeout(Duration::from_millis(100))
        .open()?;

    let read_buffer = Arc::new(Mutex::new(String::new()));

    // READING TASK
    let tr1 = read_port_buffer_task(&port, read_buffer.clone())?;

    let initial_state = Arc::new(Mutex::new(WifiState::Ready));
    let tr2 = user_input_task(
        initial_state.clone(),
        &port,
        &event_loop,
        read_buffer.clone(),
        args,
    );

    // Register handlers / state transitions
    register_event_handlers(&event_loop, read_buffer.clone(), &port)?;

    // WAIT CLOSE CONFIRM ? INVALID ? Maybe add these

    event_loop.start(initial_state.clone());

    tr1.join().map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Read Port Buffer Task failed: {:?}", e),
        )
    })?;
    let res = tr2?.join().map_err(|_e| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "Something went wrong in user input task",
        )
    })?;
    match res {
        Ok(_) => {
            println!("Returned without errors");
        }
        Err(_e) => {
            println!("Something went wrong somewhere, if my code were better, we would know where");
        }
    }

    Ok(())
}
