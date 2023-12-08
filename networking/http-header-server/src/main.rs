use anyhow::Error;
use serde_json::{Map, Value};
use socket2::{Domain, Socket, Type};
use std::mem::{self, MaybeUninit};
use std::net::SocketAddr;
use std::str;

fn main() -> Result<(), Error> {
    // Create a socket to listen for incoming IPv4 TCP packets.
    // This is accomplished with the socket(2) system call with a provided communication domain
    // and socket type. See `man socket` for a list of valid domains and types.
    //
    // IPV4:   Internet version 4 protocols
    // STREAM: Reliable, two-way connection (TCP) streams
    let listener = Socket::new(Domain::IPV4, Type::STREAM, None)?;

    // Bind the address to the socket.
    {
        // Use 0.0.0.0 to allow connections from other addresses on the network.
        let address: SocketAddr = "0.0.0.0:1337".parse().unwrap();
        listener.bind(&address.into())?;
    }

    // Indicate that the socket is ready to accept incoming connections with a maximum of 256
    // pending connections waiting in the queue.
    listener.listen(256)?;

    // Create a buffer to read bytes from the network into.
    let mut buf: [MaybeUninit<u8>; 1024] = unsafe { MaybeUninit::uninit().assume_init() };
    loop {
        // Accept a connection on the socket.
        let (sock, _addr) = listener.accept()?;

        // Read data (up to 1024 bytes) from the socket.
        // TODO: Continuously read until reaching the first empty line.
        let n = sock.recv(&mut buf)?;

        // Convert the buffer into a usable &[u8].
        let buf: &[u8] = &unsafe { mem::transmute::<_, [u8; 1024]>(buf) }[..n];

        // Parse the data as UTF-8, split by lines, then convert the headers into a JSON Map.
        let headers: Map<String, Value> = str::from_utf8(buf)?
            .lines()
            .take_while(|line| !line.is_empty())
            .filter_map(|line| line.split_once(": "))
            .map(|(key, value)| (key.into(), value.into()))
            .collect();

        // Send the response back using the same socket connection. We first send a HTTP header to
        // tell the client we are speaking HTTP as it expects. We also send a blank line to
        // indicate there are no more HTTP headers, and the JSON response will be in the HTTP body.
        let _ = sock.send(b"HTTP/1.1 200 OK\r\n\r\n")?;
        let _ = sock.send(serde_json::to_string_pretty(&headers)?.as_bytes())?;
    }
}
