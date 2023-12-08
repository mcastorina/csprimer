use anyhow::Error;
use socket2::{Domain, Socket, Type};
use std::mem::{self, MaybeUninit};
use std::net::SocketAddr;
use std::str;

const MAX_DGRAM_SIZE: usize = u16::MAX as usize;

fn main() -> Result<(), Error> {
    // Create a socket to listen for incoming IPv4 UDP packets.
    // This is accomplished with the socket(2) system call with a provided communication domain
    // and socket type. See `man socket` for a list of valid domains and types.
    //
    // IPV4:  Internet version 4 protocols
    // DGRAM: Datagram (UDP) for connectionless and unreliable messages
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;

    // Bind the address to the socket.
    {
        // Use 0.0.0.0 to allow connections from other addresses on the network.
        let address: SocketAddr = "0.0.0.0:1337".parse().unwrap();
        socket.bind(&address.into())?;
    }

    // Create a buffer to read bytes from the network into. We use MAX_DGRAM_SIZE because recv_from
    // truncates the message to the size of the buffer, and we would lose data otherwise.
    let mut buf: [MaybeUninit<u8>; MAX_DGRAM_SIZE] = unsafe { MaybeUninit::uninit().assume_init() };
    loop {
        // Wait and read data from the socket.
        let (n, addr) = socket.recv_from(&mut buf)?;
        // Convert the MaybeUninit to a &[u8] and then an uppercased String.
        let buf: &[u8] = &unsafe { mem::transmute::<_, [u8; MAX_DGRAM_SIZE]>(buf) }[..n];
        let s = str::from_utf8(buf)?.to_uppercase();
        // Send the response back to the address.
        socket.send_to(s.as_bytes(), &addr)?;
    }
}
