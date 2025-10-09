use std::{error::Error, sync::{Arc, Mutex}, thread::{self, JoinHandle}};

use serialport::SerialPort;

pub fn read_port_buffer_task(
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
