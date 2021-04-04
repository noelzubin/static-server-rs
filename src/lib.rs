use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;

#[derive(Clone)]
pub struct Server {
    allowed_exts: Option<Vec<String>>,
    prefix: String,
    root: PathBuf,
}

impl Server {
    pub fn builder() -> ServerBuilder {
        ServerBuilder::default()
    }

    pub fn run(self) {
        let listener = TcpListener::bind("127.0.0.1:8000").unwrap();
        println!("listening for connections at 8000");

        for stream in listener.incoming() {
            let server = self.clone();
            match stream {
                Ok(stream) => {
                    thread::spawn(move || {
                        server.handle_client(stream).unwrap();
                    });
                }
                Err(e) => {
                    println!("unable to connect: {}", e);
                }
            }
        }
    }

    fn handle_client(self, mut stream: TcpStream) -> io::Result<()> {
        let mut read_stream = BufReader::new(&stream);
        let mut req = String::new();
        read_stream.read_line(&mut req).unwrap();
        let (method, path) = parse_request(req);

        // validate request
        assert_eq!(method, "GET");
        let path = process_path(path, &self.allowed_exts, &self.prefix, &self.root).unwrap();

        match File::open(path) {
            Ok(file) => {
                let mut buf_reader = BufReader::new(file);
                stream.write_all(OK_HEADER.as_bytes())?;
                io::copy(&mut buf_reader, &mut stream)?;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                stream.write_all(NOT_FOUND_HEADER.as_bytes())?;
            }
            Err(_) => {
                stream.write_all(SERVER_ERR_HEADER.as_bytes())?;
            }
        }

        stream.flush().unwrap();

        Ok(())
    }
}

#[derive(Default)]
pub struct ServerBuilder {
    allowed_exts: Option<Vec<String>>,
    prefix: Option<String>,
    root: Option<PathBuf>,
}

impl ServerBuilder {
    pub fn allow_ext(mut self, exts: &[&str]) -> Self {
        self.allowed_exts = Some(exts.iter().map(|s| s.to_string()).collect());
        self
    }

    pub fn prefix<T>(mut self, prefix: T) -> Self
    where
        T: Into<String>,
    {
        self.prefix = Some(prefix.into());
        self
    }

    pub fn root<T>(mut self, root: T) -> Self
    where
        T: Into<String>,
    {
        self.root = Some(PathBuf::from(root.into()));
        self
    }

    pub fn build(self) -> Server {
        Server {
            allowed_exts: self.allowed_exts,
            prefix: self.prefix.expect("prefix is required"),
            root: self.root.expect("root is required"),
        }
    }

    pub fn run(self) {
        let server = self.build();
        server.run()
    }
}

const OK_HEADER: &str = "HTTP/1.1 200 OK\r\n\r\n";
const NOT_FOUND_HEADER: &str = "HTTP/1.1 404 ServerError\r\n\r\n";
const SERVER_ERR_HEADER: &str = "HTTP/1.1 400 ServerError\r\n\r\n";

fn parse_request(req: String) -> (String, PathBuf) {
    let mut parts = req.split(' ');
    let method = parts.next().unwrap().to_string();
    let path = parts.next().unwrap().trim();
    let path = PathBuf::from(path);
    (method, path)
}

const PREFIX_ERR: &str = "prefix not found in url";
const EXTENSION_MISMATCH_ERR: &str = "path extension doesn't match allowed values";

fn process_path(
    path: PathBuf,
    allowed_exts: &Option<Vec<String>>,
    prefix: &str,
    root: &PathBuf,
) -> Result<PathBuf, &'static str> {
    let path = path.strip_prefix(prefix).map_err(|_| PREFIX_ERR)?;

    if let Some(allowed_exts) = allowed_exts {
        let extension = &path
            .extension()
            .ok_or(EXTENSION_MISMATCH_ERR)?
            .to_str()
            .unwrap()
            .to_string();
        dbg!(&extension);
        if !allowed_exts.contains(extension) {
            return Err(EXTENSION_MISMATCH_ERR);
        }
    };

    // build final path
    Ok(Path::new(root).join(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let path = process_path(
            PathBuf::from("/prefix/some/url.jpg"),
            &Some(vec!["jpg".into()]),
            "/prefix",
            &PathBuf::from("/root"),
        );

        assert_eq!(path, Ok(PathBuf::from("/root/some/url.jpg")));
    }

    #[test]
    fn validates_ext() {
        let path = process_path(
            PathBuf::from("/prefix/some/url.png"),
            &Some(vec!["jpg".into()]),
            "/prefix",
            &PathBuf::from("/root"),
        );

        assert_eq!(path, Err(EXTENSION_MISMATCH_ERR));
    }

    #[test]
    fn validates_prefix() {
        let path = process_path(
            PathBuf::from("/not-prefix/some/url.jpg"),
            &Some(vec!["jpg".into()]),
            "/prefix",
            &PathBuf::from("/root"),
        );

        assert_eq!(path, Err(PREFIX_ERR));
    }
}
