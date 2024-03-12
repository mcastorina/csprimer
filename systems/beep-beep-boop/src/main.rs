use std::io::{self, Read, Write};
use std::time::Duration;
use std::{process, thread};
use termios::{tcsetattr, Termios, ECHO, ICANON, TCSANOW};

fn main() {
    let stdin = 0;
    let mut termios = match Termios::from_fd(stdin) {
        Ok(termios) => termios,
        Err(_) => {
            eprintln!("please run this program interactively");
            process::exit(1);
        }
    };

    let cleanup = move || {
        // Reset stdin to original termios values and exit.
        tcsetattr(stdin, TCSANOW, &termios).unwrap();
        process::exit(0);
    };
    ctrlc::set_handler(cleanup).unwrap();

    // Disable echo and canonical mode.
    termios.c_lflag &= !(ICANON | ECHO);
    tcsetattr(stdin, TCSANOW, &termios).unwrap();

    let stdout = io::stdout();
    let mut reader = io::stdin();
    let mut buffer = [0; 1];
    loop {
        reader.read_exact(&mut buffer).unwrap();
        if buffer[0].is_ascii_digit() {
            let count = buffer[0] - 0x30;
            for _ in 0..count {
                print!("\x07");
                stdout.lock().flush().unwrap();
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}
