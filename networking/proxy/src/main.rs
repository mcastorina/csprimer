use anyhow::Error;
use http_body_util::{Either, Empty};
use hyper::{
    body::{Body, Bytes, Incoming},
    client::conn::http1 as client,
    server::conn::http1 as server,
    service::service_fn,
    Request, Response, StatusCode,
};
use hyper_util::rt::tokio::TokioIo;
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create a TCP socket that's listening for incoming connections.
    // Use 0.0.0.0 to allow connections from other addresses on the network.
    let listener = TcpListener::bind("0.0.0.0:1337").await?;
    loop {
        // Wait for a client to connect.
        let (tcp, _addr) = listener.accept().await?;
        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(tcp);

        // Spin up a new task in Tokio so we can continue to listen for new TCP connection on the
        // current task without waiting for the processing of the HTTP1 connection we just received
        // to finish.
        tokio::task::spawn(async move {
            // Handle the connection from the client using HTTP1 and pass any
            // HTTP requests received on that connection to the `proxy` function.
            if let Err(err) = server::Builder::new()
                .preserve_header_case(true)
                .serve_connection(io, service_fn(proxy))
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

// proxy uses hyper to abstract a basic HTTP proxy service. hyper does the heavy lifting of
// serialization by providing a Request object, and we reply with a Response object.
async fn proxy<H: Body + Send>(
    req: Request<H>,
) -> Result<Response<Either<Empty<Bytes>, Incoming>>, Error>
where
    <H as Body>::Data: Send,
    <H as Body>::Error: std::error::Error + Sync + Send,
{
    println!(" -> *    {} {}", req.method(), req.uri());

    // Create a new TCP connection to the upstream server. If it fails, respond with a 502 Bad
    // Gateway to the downstream request.
    let upstream = match TcpStream::connect("127.0.0.1:8000").await {
        Ok(upstream) => upstream,
        Err(_) => {
            println!(" <- *    502 Bad Gateway");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Either::Left(Empty::new()))?);
        }
    };
    let io = TokioIo::new(upstream);

    // Perform a TCP handshake with the upstream server.
    let (mut sender, conn) = client::handshake(io).await?;
    // Spawn a task to poll the connection which enables the actual sending and receiving of bytes.
    tokio::task::spawn(async {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });
    println!("    * -> {} {}", req.method(), req.uri());
    // Send the exact same downstream request to the upstream and wait for the response.
    let resp = sender.send_request(req).await?;
    println!("    * <- {}", resp.status());
    println!(" <- *    {}", resp.status());
    // Return the response from the upstream to the downstream client.
    Ok(resp.map(Either::Right))
}
