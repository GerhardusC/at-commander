
use clap::{Parser, command};

/// Simple program to communicate AT commands with the ESP-01 module.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Port
    #[arg(short, long, default_value_t = String::from("/dev/ttyUSB0"))]
    pub port: String,

    /// Baud rate
    #[arg(short, long, default_value_t = 115_200)]
    pub baud_rate: u32,

    /// By ending an input string with the "~" character, you may specify to send a buffer
    /// instead of an ASCII string. This argument determines the base in which your buffer will
    /// be interpreted. By default it is 16, i.e. your buffers need to be HEX values split by
    /// spaces. E.G. "0F F0 30 40 5C~" etc.
    #[arg(short, long, default_value_t = 16)]
    pub radix_input_buffer: u8,
}

