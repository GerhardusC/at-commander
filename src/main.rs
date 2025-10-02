use std::{
    error::Error,
    io::stdin,
    thread::{self},
    time::Duration,
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut port = serialport::new("/dev/ttyUSB0", 115_200)
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
                Err(e) => eprintln!("{:?}", e),
            }
        }
    });

    loop {
        let mut input = String::new();
        let _bytes_read = stdin().read_line(&mut input)?;
        let res = port.write((input.trim().to_owned() + "\r\n").as_bytes());
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
