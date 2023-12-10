use anyhow::{anyhow, Error};
use std::fmt::Display;
use std::{
    env,
    io::{Cursor, Write},
    net::UdpSocket,
};

fn main() -> Result<(), Error> {
    // Get the domain from the first CLI argument.
    let domain = env::args()
        .nth(1)
        .ok_or(anyhow!("please provide a domain"))?;

    // Create a socket to send and receive UDP packets.
    let socket = UdpSocket::bind(("0.0.0.0", 1337))?;
    // We are only querying 1.1 (equivalent to 1.0.0.1) which is Cloudflare's nameserver.
    socket.connect(("1.1", 53))?;

    // Create a buffer to read/write.
    let mut buf = Cursor::new([0; 2048]);

    // Encode a query for miccah.io into the buffer and send it to the nameserver.
    let n = Query(&domain).encode(&mut buf)?;
    socket.send(&buf.get_ref()[..n])?;

    // Read and parse a response from the nameserver.
    let n = socket.recv(buf.get_mut())?;
    let answers = Answers::try_from(&buf.get_ref()[..n])?;

    // Display the answers.
    println!("Response for {domain}");
    for answer in answers {
        println!("{}", answer);
    }
    Ok(())
}

// Simplified DNS query for a provided domain's A record.
struct Query<T: AsRef<str>>(T);

impl<T: AsRef<str>> Query<T> {
    // Encode the query as bytes.
    fn encode(&self, mut w: impl Write) -> Result<usize, Error> {
        Ok(Self::encode_header(&mut w)? + self.encode_question(&mut w)?)
    }

    // Write a hard-coded header as bytes. The header indicates we are only asking one question
    // with recursion desired set.
    fn encode_header(mut w: impl Write) -> Result<usize, Error> {
        //                                 1  1  1  1  1  1
        //   0  1  2  3  4  5  6  7  8  9  0  1  2  3  4  5
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                      ID                       |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |QR|   Opcode  |AA|TC|RD|RA|   Z    |   RCODE   |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                    QDCOUNT                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                    ANCOUNT                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                    NSCOUNT                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                    ARCOUNT                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        w.write_all(&[
            5, 57, // ID
            1, 0, // Header flags: RD (recursion desired)
            0, 1, // Question count: 1
            0, 0, // Answer count: 0
            0, 0, // Name server count: 0
            0, 0, // Additional record count: 0
        ])?;
        Ok(12)
    }

    // Encode the question as bytes, assuming an A record and IN class.
    fn encode_question(&self, mut w: impl Write) -> Result<usize, Error> {
        //                                 1  1  1  1  1  1
        //   0  1  2  3  4  5  6  7  8  9  0  1  2  3  4  5
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                                               |
        // /                     QNAME                     /
        // /                                               /
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                     QTYPE                     |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                     QCLASS                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        let domain = self.0.as_ref();
        let mut nbytes = 0;
        for octet in domain.split('.') {
            let octet_length = u8::try_from(octet.len())?;
            let octet_bytes = octet.as_bytes();
            w.write_all(&[octet_length])?;
            w.write_all(octet_bytes)?;
            nbytes += octet_bytes.len() + 1;
        }
        w.write_all(&[0])?;
        nbytes += 1;
        w.write_all(&[
            0, 1, // Question type: A
            0, 1, // Question class: IN (internet)
        ])?;
        nbytes += 4;
        Ok(nbytes)
    }
}

#[derive(Debug)]
struct Answers(Vec<Answer>);

#[derive(Debug)]
struct Answer {
    r#type: u16,
    class: u16,
    ttl: u32,
    address: [u8; 4],
}

impl TryFrom<&[u8]> for Answers {
    type Error = Error;

    // Parse a byte buffer into a list of Answers. This expects the entire DNS response, so it can
    // parse the header for the count of answers. This also assumes each answer has the same size
    // and that the answer names are compressed to 2 bytes.
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        //                                 1  1  1  1  1  1
        //   0  1  2  3  4  5  6  7  8  9  0  1  2  3  4  5
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                      ID                       |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |QR|   Opcode  |AA|TC|RD|RA|   Z    |   RCODE   |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                    QDCOUNT                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                    ANCOUNT                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                    NSCOUNT                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                    ARCOUNT                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        let num_answers = u16::from_be_bytes([
            *value.get(6).ok_or(anyhow!("not enough bytes"))?,
            *value.get(7).ok_or(anyhow!("not enough bytes"))?,
        ]);
        const ANSWER_SIZE: usize = 16;
        let answer_start = value.len() - (num_answers as usize * ANSWER_SIZE);
        let answer_bytes: Vec<&[u8]> = (0..num_answers as usize)
            .map(|ofs| {
                let start = answer_start + ofs * ANSWER_SIZE;
                value.get(start..start + ANSWER_SIZE)
            })
            .collect::<Option<_>>()
            .ok_or(anyhow!("not enough bytes"))?;
        Ok(Answers(
            answer_bytes
                .into_iter()
                .map(Answer::try_from)
                .collect::<Result<_, _>>()?,
        ))
    }
}

impl TryFrom<&[u8]> for Answer {
    type Error = Error;

    // Parse a byte buffer into a single Answer. The name is assumed to be compressed to 2 bytes,
    // and the data is assumed to be 4 bytes.
    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        //                                 1  1  1  1  1  1
        //   0  1  2  3  4  5  6  7  8  9  0  1  2  3  4  5
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                                               |
        // /                                               /
        // /                      NAME                     /
        // |                                               |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                      TYPE                     |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                     CLASS                     |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                      TTL                      |
        // |                                               |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        // |                   RDLENGTH                    |
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--|
        // /                     RDATA                     /
        // /                                               /
        // +--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+--+
        let get_u16 = |ofs: usize| -> Result<u16, Error> {
            Ok(u16::from_be_bytes([
                *value.get(ofs).ok_or(anyhow!("not enough bytes"))?,
                *value.get(ofs + 1).ok_or(anyhow!("not enough bytes"))?,
            ]))
        };
        let get_u32 = |ofs: usize| -> Result<u32, Error> {
            Ok(u32::from_be_bytes([
                *value.get(ofs).ok_or(anyhow!("not enough bytes"))?,
                *value.get(ofs + 1).ok_or(anyhow!("not enough bytes"))?,
                *value.get(ofs + 2).ok_or(anyhow!("not enough bytes"))?,
                *value.get(ofs + 3).ok_or(anyhow!("not enough bytes"))?,
            ]))
        };
        Ok(Answer {
            r#type: get_u16(2)?,
            class: get_u16(4)?,
            ttl: get_u32(6)?,
            address: get_u32(12)?.to_be_bytes(),
        })
    }
}

impl Display for Answer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}.{} (TTL = {})",
            self.address[0], self.address[1], self.address[2], self.address[3], self.ttl
        )
    }
}

// Convenience trait implementation so we can iterate over the Answers type.
impl IntoIterator for Answers {
    type Item = Answer;
    type IntoIter = <Vec<Answer> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
