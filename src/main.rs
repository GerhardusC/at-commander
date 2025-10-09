use std::{
    error::Error,
    sync::{Arc, Mutex},
    time::Duration,
};

use at_commander::{args::Args, event_handlers::register_event_handlers, event_loop::{EventLoop, WifiState}, port_input::read_port_buffer_task, user_input::user_input_task};
use clap::Parser;

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
