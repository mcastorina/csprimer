use anyhow::Error;
use std::net::{TcpListener, TcpStream};
use std::{io, thread};

pub fn main() -> Result<(), Error> {
    // Create a TCP socket that's listening for incoming connections.
    // Use 0.0.0.0 to allow connections from other addresses on the network.
    let listener = TcpListener::bind("0.0.0.0:1337")?;
    loop {
        // Wait for a client to connect.
        let (downstream, _addr) = listener.accept()?;

        // Spawn a thread to handle the proxy for this connection.
        thread::spawn(|| {
            let upstream = TcpStream::connect("127.0.0.1:8000")?;
            proxy(downstream, upstream)
        });
    }
}

fn proxy(mut down: TcpStream, mut up: TcpStream) -> Result<(), Error> {
    let mut down_reader = down.try_clone()?;
    let mut up_writer = up.try_clone()?;
    let handle = thread::spawn(move || -> Result<(), Error> {
        io::copy(&mut down_reader, &mut up_writer)?;
        Ok(())
    });
    io::copy(&mut up, &mut down)?;
    handle.join().unwrap()?;
    Ok(())
}
