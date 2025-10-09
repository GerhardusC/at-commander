use std::{error::Error, sync::{Arc, Mutex}, thread::{self, sleep}, time::Duration};

use serialport::SerialPort;

use crate::event_loop::{EventLoop, TrackWifiState, WifiEvent, WifiState};


pub fn register_event_handlers(
    event_loop: &EventLoop,
    read_buffer: Arc<Mutex<String>>,
    port: &Box<dyn SerialPort>,
) -> Result<(), Box<dyn Error>> {
    // Register handlers / state transitions
    let mut port_cp = port.try_clone()?;
    event_loop.on(WifiEvent::Configure, move |_e, state| {
        let current_state = state.get();
        if let WifiState::Ready = current_state {
            if let Err(e) = port_cp.write("ATE0\r\n".as_bytes()) {
                println!("Failed to write message to wifi device: {e}");
            };
        } else {
            println!("Invalid state for configure request: {:?}", current_state);
        }
    });

    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::Reset, move |_e, state| {
        // Clear port read string.
        if let Ok(mut read_buffer) = read_buffer_cp.lock() {
            read_buffer.clear();
        };
        let _ = port_cp.flush();

        if let Err(e) =  port_cp.write(&[0xE0, 0x00]) {
            println!("Failed to write message to wifi device: {e}");
        }
        if let Err(e) =  port_cp.write("AT+CIPCLOSE\r\n".as_bytes()) {
            println!("Failed to write message to wifi device: {e}");
        }

        println!("State reset to Ready and we tried everything in our power to reset the connection");
        state.change_to(WifiState::Ready);
    });

    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::PublishConnectRequest, move |e, state| {
        let current_state = state.get();
        if let WifiState::Ready = current_state {
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
        } else {
            println!("Invalid state for publish request: {:?}", current_state);
        }
    });

    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::ConnAck, move |_e, state| {
        let current_state = state.get();
        if let WifiState::WaitingConnectAck = current_state {
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
                        if timeout > 1000 {
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
                        println!("Failed to write message to wifi device: {e}");
                        state.change_to(WifiState::Ready);
                    }
                    let _ = port_cp.flush();

                    state.change_to(WifiState::Connected);
                });
            }
        } else {
            println!("Invalid state for ConnAck request: {:?}", current_state);
        }
    });

    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::Publish, move |e, state| {
        let current_state = state.get();
        if let WifiState::Connected = current_state {
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
                println!("Failed to write message to wifi device: {e}");
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
                        println!("Failed to write message to wifi device: {e}");
                        state.change_to(WifiState::Ready);
                    };
                    let _ = port_cp.flush();

                    state.change_to(WifiState::WaitingPublishAck);
                });
            }
        } else {
            println!("Invalid state for Publish request: {:?}", current_state);
        }
    });

    let mut port_cp = port.try_clone()?;
    let read_buffer_cp = read_buffer.clone();
    event_loop.on(WifiEvent::AckReceived, move |_e, state| {
        let current_state = state.get();
        if let WifiState::WaitingPublishAck = current_state {
            if let Ok(mut read_buffer) = read_buffer_cp.lock() {
                read_buffer.clear();
            };
            // Tell how many bytes will be sent
            if let Err(e) = port_cp.write("AT+CIPSEND=2\r\n".as_bytes()) {
                println!("Something went wrong writing cpsend to port: {e}");
                return;
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
                                // Port ready to receive connect request.
                                break;
                            }
                        }

                        timeout += 1;
                        sleep(Duration::from_millis(1));
                    }
                    // Send bytes
                    if let Err(e) = port_cp.write(&[0xE0, 0x00]) {
                        println!("Something went wrong writing cpsend to port: {e}");
                        state.change_to(WifiState::Ready);
                        return;
                    };
                    state.change_to(WifiState::Ready);
                });
            }
        } else {
            println!("Invalid state for AckRecieved request: {:?}", current_state);
        }

    });
    Ok(())
}
