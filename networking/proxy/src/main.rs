use anyhow::Error;
use core::future::Future;
use http_body_util::{Either, Empty};
use hyper::{
    body::{Bytes, Incoming},
    client::conn::http1::{self as client, SendRequest},
    server::conn::http1 as server,
    service::Service,
    Request, Response, StatusCode,
};
use hyper_util::rt::tokio::TokioIo;
use std::pin::Pin;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create a TCP socket that's listening for incoming connections.
    // Use 0.0.0.0 to allow connections from other addresses on the network.
    let listener = TcpListener::bind("0.0.0.0:1337").await?;
    // Create a persistent upstream connection to proxy requests to.
    let upstream = upstream_handshake().await?;
    let upstream = Arc::new(Mutex::new(upstream));
    loop {
        // Wait for a client to connect.
        let (tcp, _addr) = listener.accept().await?;
        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        let io = TokioIo::new(tcp);

        // Spin up a new task in Tokio so we can continue to listen for new TCP connection on the
        // current task without waiting for the processing of the HTTP1 connection we just received
        // to finish.
        let upstream = Arc::clone(&upstream);
        tokio::task::spawn(async move {
            // Handle the connection from the client using HTTP1 and pass any
            // HTTP requests received on that connection to the `proxy` function.
            if let Err(err) = server::Builder::new()
                .preserve_header_case(true)
                .serve_connection(io, Proxy { upstream })
                .await
            {
                println!("Error serving connection: {:?}", err);
            }
        });
    }
}

struct Proxy {
    upstream: Arc<Mutex<SendRequest<Incoming>>>,
}

type ProxyResponseOr502 = Either<Empty<Bytes>, Incoming>;

impl Service<Request<Incoming>> for Proxy {
    type Response = Response<ProxyResponseOr502>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        let resp = proxy(Arc::clone(&self.upstream), req);
        Box::pin(resp)
    }
}

// proxy uses hyper to abstract a basic HTTP proxy service. hyper does the heavy lifting of
// serialization by providing a Request object, and we reply with a Response object.
async fn proxy(
    sender: Arc<Mutex<SendRequest<Incoming>>>,
    req: Request<Incoming>,
) -> Result<Response<ProxyResponseOr502>, Error> {
    println!(" -> *    {} {}", req.method(), req.uri());
    // Get exclusive access to the upstream TCP connection.
    let mut sender = sender.lock().await;

    println!("    * -> {} {}", req.method(), req.uri());
    // Send the exact same downstream request to the upstream and wait for the response.
    // In the event of an error, the most likely cause is the TCP stream ended, so attempt to
    // recreate it. Unfortunately, we cannot retry the request because it is consumed by hyper (the
    // body being possibly streamed), so we'll leave it to the client to retry.
    let resp = match sender.send_request(req).await {
        Ok(resp) => resp,
        Err(err) => {
            println!("    * <- Error: {err}");
            *sender = upstream_handshake().await?;
            println!(" <- *    502 Bad Gateway");
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Either::Left(Empty::new()))?);
        }
    };
    println!("    * <- {}", resp.status());
    println!(" <- *    {}", resp.status());
    // Return the response from the upstream to the downstream client.
    Ok(resp.map(Either::Right))
}

// upstream_handshake initialized a TCP connection with the upstream server and returns a handle
// for sending requests.
async fn upstream_handshake() -> Result<SendRequest<Incoming>, Error> {
    let upstream = TcpStream::connect("127.0.0.1:8000").await?;
    let (sender, conn) = client::handshake(TokioIo::new(upstream)).await?;
    // Spawn a task to poll the connection which enables the actual sending and receiving of bytes.
    tokio::task::spawn(async {
        if let Err(err) = conn.await {
            println!("Connection failed: {:?}", err);
        }
    });
    Ok(sender)
}
