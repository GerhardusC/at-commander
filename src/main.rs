use std::{
    error::Error, io::stdin, thread::{self, sleep}, time::Duration
};

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
}

fn parse_hex_bytes(input: &str) -> Result<Vec<u8>, String> {
    input
        .split_whitespace()
        .map(|token| u8::from_str_radix(token, 16)
        .map_err(|_| format!("Invalid hex: {}", token)))
        .collect()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut port = serialport::new(&args.port, args.baud_rate)
        .timeout(Duration::from_millis(100))
        .open()?;

    let mut port_cp1 = port.try_clone()?;
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

    loop {
        let mut input = String::new();
        let _bytes_read = stdin().read_line(&mut input)?;

        let mut trimmed_input = input.trim().to_owned();

        let payload = if trimmed_input.ends_with("~") {
            trimmed_input.pop();
            let bytes_vec = parse_hex_bytes(&trimmed_input)?;
            println!("SENDING:{:?}", bytes_vec);
            sleep(Duration::from_millis(10));
            bytes_vec
        } else {
            (trimmed_input.to_owned() + "\r\n").as_bytes().to_owned()
        };

        let res = port.write(payload.as_slice());
        port.flush()?;
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

    tr1.join().map_err(|_e| {
        std::io::Error::new(std::io::ErrorKind::Other, format!("Something went wrong."))
    })?;

    Ok(())
}
