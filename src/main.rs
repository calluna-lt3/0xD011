/* resources:
 * https://github.com/rustls/hyper-rustls/blob/cb05f463ed7d34e18fa580fd7249eddebcc39ec5/examples/server.rs
 * https://hyper.rs/guides/1/server/hello-world/
 * https://docs.rs/hyper/latest/hyper/
 * https://docs.rs/tokio/latest/tokio/
 *
 */

// NOTE: check check check
// * https://github.com/theseus-rs/file-type/blob/main/FILETYPES.md
use std::convert::Infallible;
use std::fs::File;
use std::io::{Read, Write};
use std::net::SocketAddr;

use http_body_util::{BodyExt, Full};
use hyper::body::{Body, Bytes};
use hyper::header::HeaderValue;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use std::net::Ipv4Addr;
use std::path::Path;
use tokio::net::TcpListener;

use sqlx::migrate::MigrateDatabase;
use sqlx::{Row, Sqlite, SqlitePool};

const DB_URL: &str = "sqlite://sqlite.db";
const FILESIZE_MAX: u64 = 2048;

// TODO: proper templates
// https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Status
#[macro_export]
macro_rules! template_content {
    ($x:expr) => {
        match $x {
            404 => Bytes::from("404"),
            _ => {
                panic!("idunno what happened sorry");
            }
        }
    };
}

#[allow(dead_code)]
fn print_packet(req: &Request<hyper::body::Incoming>) {
    let mut headers = String::new();
    for (k, v) in req.headers() {
        headers.push_str(format!("{k:?}: {v:?}\n").as_str());
    }

    let method = req.method();
    let uri = req.uri();
    let res = format!(
        r#">>>>>>>>>>>>>>>>>>>>
 method: {method:?}
    uri: {uri}
===== HEADERS START =====
{headers}
===== HEADERS END =====
<<<<<<<<<<<<<<<<<<<<"#
    );
    println!("{res}");
}

async fn handle_get(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let content = match req.uri().path() {
        "/" => {
            let file = File::open("./pages/index.html");
            if let Ok(file) = file {
                // TODO: change this to return a 404 if any error exists
                file.bytes().map(|x| x.unwrap_or(b' ')).collect()
            } else {
                template_content!(404)
            }
        }
        "/styles.css" => {
            let file = File::open("./pages/styles.css");
            if let Ok(file) = file {
                file.bytes().map(|x| x.unwrap_or(b' ')).collect()
            } else {
                template_content!(404)
            }
        }
        other => {
            let file = File::open(format!("./arbitrary{other}"));
            if let Ok(file) = file {
                file.bytes().map(|x| x.unwrap_or(b' ')).collect()
            } else {
                template_content!(404)
            }
        }
    };

    Ok(Response::new(Full::new(content)))
}

fn boundary_from_content_type(content_type: &HeaderValue) -> Option<String> {
    let content_type = content_type.to_str().unwrap();
    let boundary_start = content_type.find("boundary");
    if let Some(start) = boundary_start {
        // remove 'boundary=' from the range
        let start = start + 9;
        let mut chars = (&content_type[start..]).chars();
        let mut end = start;

        // find semicolon or EOL
        loop {
            let char = chars.next();
            if let None = char {
                break;
            }

            let char = char.unwrap();
            if let ';' = char {
                break;
            }

            end += 1;
        }

        // boundary starts with '--'
        Some(format!("--{}", &content_type[start..end]))
    } else {
        None
    }
}

fn write_bytes(file: &mut File, bytes: Bytes) -> Result<(), std::io::Error> {
    file.write_all(bytes.to_vec().as_slice())
}

async fn handle_post(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let (parts, data) = req.into_parts();
    let _host = parts.headers.get("host").unwrap();

    // we only want to accept forms
    // * https://www.iana.org/assignments/media-types/media-types.xhtml
    let content_type = parts.headers.get("content-type").unwrap();
    let content_type_str = content_type.to_str().unwrap();
    if let Some(i) = content_type_str.find(";") {
        assert_eq!("multipart/form-data", &content_type_str[..i]);
    } else {
        panic!("how are we here");
    }

    let boundary = boundary_from_content_type(&content_type);
    if let None = boundary {
        panic!("no boundary field");
    }

    let boundary = boundary.unwrap();
    let boundary_len = boundary.len();

    // TODO db: hash files => filename, (!!! KEEP EXTENSION !!!)
    let mut file = File::create("./arbitrary/what.txt").unwrap();

    // TODO: proper size checking, this is currently just an estimated upper bound
    let size_hint = data.size_hint();
    match size_hint.upper() {
        Some(size) => {
            if size > FILESIZE_MAX {
                todo!("dropped packet, size hint upper bound too big: `{size}`");
            }
        }
        None => {
            todo!("dropped packet, size hint upper bound non-existent");
        }
    }

    // NOTE: might already be loaded? packet size limit via firewall might be best ,, idk
    // read entire body of the request into memory
    let body = data.collect().await.unwrap().to_bytes();

    // verify boundary matches what was in header
    assert_eq!(body.slice(0..boundary_len), boundary);

    // skip header included in body by finding first instance of '\r\n\r\n'
    // NOTE: what if there is '\r\n\r\n' sent in the headers?

    // (progress : expect) = (0 | 2 : '\r') || (1 : '\n')
    let mut progress = 0;
    let mut expect = b'\r';
    let mut header_end: Option<usize> = None;
    for (i, byte) in body.slice(boundary_len..).iter().enumerate() {
        if *byte == expect {
            progress += 1;
            expect = if progress == 2 { b'\r' } else { b'\n' }
        } else {
            progress = 0;
            expect = b'\r';
        }

        if progress == 3 {
            header_end = Some(i);
            break;
        }
    }

    if let None = header_end {
        panic!("no header end found");
    }

    let header_end = header_end.unwrap() + boundary_len + 2;

    // TODO db: db entry, probably just hash files and store on fs with the
    // hashes as an entry. binary blobs wouldnt be that bad if it stays small
    // but if there was ever real amounts of traffic this would be an issue
    // * https://dba.stackexchange.com/questions/2445/should-binary-files-be-stored-in-the-database

    // write to file
    // 6 is from format of '\r\n<boundary>--\r\n'
    let rest_body = body.slice(header_end..(body.len() - boundary_len - 6));
    // println!("{:?}", infer::get(&rest_body));
    write_bytes(&mut file, rest_body).unwrap();

    Ok(Response::new(Full::new(template_content!(404))))
}

async fn handle_request(
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = req.method();
    match *method {
        Method::GET => handle_get(req).await,
        Method::POST => handle_post(req).await,
        _ => {
            panic!("unsupported method: {method:?}");
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct Upload {
    hash: String,
    owner: String,
    extension: String,
    time_uploaded: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 3000));
    let listener = TcpListener::bind(addr).await?;

    // db stuff u faggot
    if !Sqlite::database_exists(DB_URL).await.unwrap_or(false) {
        println!("Creating database {}", DB_URL);
        match Sqlite::create_database(DB_URL).await {
            Ok(_) => println!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    } else {
        println!("db exists");
    }

    let db = SqlitePool::connect(DB_URL).await.unwrap();
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let migrations = Path::new(&crate_dir).join("./migrations");
    let migration_res = sqlx::migrate::Migrator::new(migrations)
        .await
        .unwrap()
        .run(&db)
        .await;

    match migration_res {
        Ok(_) => println!("migration ok"),
        Err(e) => panic!("err: {}", e),
    }

    let res = sqlx::query("INSERT INTO uploads VALUES ('abcdefghabcdefghabcdefghabcdefghabcdefghabcdefghabcdefghabcdefgh', '111.111.111.111', 'txt', '2025-04-01 22:22:22');")
        .execute(&db)
        .await
        .unwrap();

    let res = sqlx::query_as::<_, Upload>("SELECT * FROM uploads")
        .fetch_all(&db)
        .await
        .unwrap();

    res.iter().for_each(|x| {
        println!("hash: {}", x.hash);
        println!("owner: {}", x.owner);
        println!("extension: {}", x.extension);
        println!("time_uploaded: {}", x.time_uploaded);
    });

    todo!();

    loop {
        let (stream, _) = listener.accept().await?;

        // Use an adapter to access something implementing `tokio::io` traits
        // as if they implement `hyper::rt` IO traits.
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
