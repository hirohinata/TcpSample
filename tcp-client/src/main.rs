use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let addr = "127.0.0.1:4000";
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    println!("Connected to {}", addr);

    let msg = b"hello from rust client\n";
    stream.write_all(msg)?;
    println!("Sent: {}", String::from_utf8_lossy(msg));

    let mut buf = Vec::new();
    let mut tmp = [0u8; 1024];
    let n = stream.read(&mut tmp)?;
    buf.extend_from_slice(&tmp[..n]);

    println!("Received: {}", String::from_utf8_lossy(&buf));
    Ok(())
}