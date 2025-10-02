use std::{error::Error, io::stdin, sync::{Arc, Mutex}, thread::{self, sleep}, time::Duration};


fn main() -> Result<(), Box<dyn Error>> {
    let mut port = serialport::new("/dev/ttyUSB0", 115_200)
        .timeout(Duration::from_millis(100))
        .open()?;

    let port_mux = Arc::new(Mutex::new(port));

    let port_cp1 = port_mux.clone();
    let tr1 = thread::spawn(move || {
        loop {
            let res = port_cp1.try_lock();
            if let Ok(mut res) = res {
                let mut buf: Vec<u8> = Vec::new();

                let res = res.read_to_end(&mut buf);
                if let Ok(x) = res {
                    if x > 0 {
                        println!("Msg: {}", String::from_utf8_lossy(&buf));
                    }
                }
            }
            sleep(Duration::from_millis(10));
        }
    });

    loop {
        let mut input = String::new();
        let _bytes_read = stdin().read_line(&mut input)?;
        println!("{input}");
        if let Ok(mut port) = port_mux.try_lock() {
            let res = port.write_all((input.trim().to_owned() + "\n\r").as_bytes());
            port.flush()?;
            match res {
                Ok(x) => {
                    // if x > 0 {
                    //     println!("Bytes written: {}", x);
                    // }
                    x
                },
                Err(e) => {
                    println!("{e}");
                    break;
                },
            }

        };
    }

    tr1.join()
        .map_err(|_e| std::io::Error::new(
            std::io::ErrorKind::Other, format!("Something went wrong."))
        )?;

    Ok(())
}
