/* resources:
 * https://github.com/rustls/hyper-rustls/blob/cb05f463ed7d34e18fa580fd7249eddebcc39ec5/examples/server.rs
 * https://hyper.rs/guides/1/server/hello-world/
 * https://docs.rs/hyper/latest/hyper/
 * https://docs.rs/tokio/latest/tokio/
 *
 */

use std::convert::Infallible;
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;

use futures_util::stream::StreamExt;
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use std::net::Ipv4Addr;

// https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status
#[macro_export]
macro_rules! template_content {
    ($x:expr) => {
        match $x {
            404 => { Bytes::from("404") },
            _ => { panic!("idunno what happened sorry") },
        }
    };
}


fn print_packet(req: &Request<hyper::body::Incoming>) {
    let mut headers = String::new();
    for (k, v) in req.headers() {
        headers.push_str(format!("{k:?}: {v:?}\n").as_str());
    }

    let method = req.method();
    let uri = req.uri();
    let res = format!(r#">>>>>>>>>>>>>>>>>>>>
 method: {method:?}
    uri: {uri}
===== HEADERS START =====
{headers}
===== HEADERS END =====
<<<<<<<<<<<<<<<<<<<<"#);
    println!("{res}");
}

async fn handle_get(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let content = match req.uri().path() {
        "/" => {
            let index = File::open("./index.html");
            if let Ok(file) = index {
                // TODO: change this to return a 404 if any error exists
                file.bytes().map(|x| x.unwrap_or(b' ')).collect()
            } else {
                template_content!(404)
            }
        },
        "/styles.css" => {
            let styles = File::open("./styles.css");
            if let Ok(file) = styles {
                file.bytes().map(|x| x.unwrap_or(b' ')).collect()
            } else {
                template_content!(404)
            }
        },
        "/bunny" => {
            Bytes::from("bunny")
        },
        _ => {
            template_content!(404)
        }
    };

    Ok(Response::new(Full::new(content)))
}

// https://www.iana.org/assignments/media-types/media-types.xhtml
async fn handle_post(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    let (parts, data) = req.into_parts();
    let host = parts.headers.get("host");
    let len = parts.headers.get("content-length");
    let rest = parts.headers.get("content-type");
    println!("{rest:?}");
    todo!()
}

async fn handle_request(req: Request<hyper::body::Incoming>) -> Result<Response<Full<Bytes>>, Infallible> {
    print_packet(&req);
    let method = req.method();
    match *method {
        Method::GET => {
            handle_get(req).await
        },
        Method::POST => {
            handle_post(req).await
        },
        _ => {
            panic!("unsupported method: {method:?}")
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 3000));
    let listener = TcpListener::bind(addr).await?;

    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits as if they implement
        // `hyper::rt` IO traits.
        // NOTE: ?????????
        let io = TokioIo::new(stream);

        tokio::task::spawn(async move {
            if let Err(err) = http1::Builder::new()
                .serve_connection(io, service_fn(handle_request))
                .await
            {
                eprintln!("Error serving connection: {:?}", err);
            }
        });
    }
}
