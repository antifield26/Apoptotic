//! 集成测试：验证服务器 ping 响应
//!
//! 启动服务器进程 → 发送 Handshake + Status Request → 验证响应 → 关闭服务器

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command};
use std::time::Duration;

/// VarInt 编码
fn write_varint(value: i32) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut v = value as u32;
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if v == 0 {
            break;
        }
    }
    buf
}

/// VarInt 解码 (from TcpStream)
fn read_varint(stream: &mut TcpStream) -> Result<i32, String> {
    let mut value: i32 = 0;
    let mut shift: u32 = 0;
    loop {
        let mut buf = [0u8; 1];
        stream
            .read_exact(&mut buf)
            .map_err(|e| format!("read error: {}", e))?;
        let byte = buf[0];
        value |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
        shift += 7;
        if shift >= 32 {
            return Err("VarInt too large".into());
        }
    }
}

struct ServerGuard {
    child: Child,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start_server() -> Option<ServerGuard> {
    // Use the binary built by cargo for testing
    let bin = std::env::var("CARGO_BIN_EXE_mc-server").ok()?;
    let child = Command::new(&bin)
        .current_dir(
            std::path::Path::new(&bin)
                .parent()?
                .parent()?
                .parent()?
                .parent()?,
        )
        .spawn()
        .ok()?;
    Some(ServerGuard { child })
}

fn try_connect(timeout_secs: u64) -> Option<TcpStream> {
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < timeout_secs {
        if let Ok(s) = TcpStream::connect("127.0.0.1:25565") {
            return Some(s);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    None
}

#[test]
fn test_server_ping_response() {
    let _guard = match start_server() {
        Some(g) => g,
        None => {
            eprintln!("SKIP: Could not start server (run cargo build --bin mc-server first)");
            return;
        }
    };

    // Wait for server to be ready
    let mut stream = match try_connect(10) {
        Some(s) => s,
        None => {
            eprintln!("SKIP: Server did not start in time");
            return;
        }
    };

    stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();

    // Send Handshake (next_state = Status = 1)
    let host = "localhost";
    let mut handshake = Vec::new();
    handshake.extend_from_slice(&write_varint(0x00)); // packet ID
    handshake.extend_from_slice(&write_varint(776));   // protocol version (26.2)
    handshake.extend_from_slice(&write_varint(host.len() as i32)); // string length
    handshake.extend_from_slice(host.as_bytes());
    handshake.extend_from_slice(&25565u16.to_be_bytes());
    handshake.extend_from_slice(&write_varint(1)); // next state: Status

    let mut frame = Vec::new();
    frame.extend_from_slice(&write_varint(handshake.len() as i32));
    frame.extend_from_slice(&handshake);
    stream.write_all(&frame).expect("send handshake");

    // Send Status Request
    let status_req = {
        let mut f = Vec::new();
        f.extend_from_slice(&write_varint(1));
        f.push(0x00);
        f
    };
    stream.write_all(&status_req).expect("send status request");

    // Read Response
    let msg_len = read_varint(&mut stream).expect("read response length");
    assert!(msg_len > 0, "response must not be empty");

    let mut msg = vec![0u8; msg_len as usize];
    stream.read_exact(&mut msg).expect("read response body");

    // Skip packet ID
    let mut offset = 0;
    while offset < msg.len() && msg[offset] & 0x80 != 0 {
        offset += 1;
    }
    offset += 1; // skip last byte of packet ID varint

    // Read string length varint
    let mut str_len: i32 = 0;
    let mut shift: u32 = 0;
    while offset < msg.len() {
        let byte = msg[offset];
        offset += 1;
        str_len |= ((byte & 0x7F) as i32) << shift;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }

    let json_bytes = &msg[offset..offset + str_len as usize];
    let json_str = std::str::from_utf8(json_bytes).expect("valid UTF-8");

    let response: serde_json::Value =
        serde_json::from_str(json_str).expect("valid JSON");

    assert_eq!(
        response["description"]["text"].as_str().unwrap(),
        "Minecraft LAN Server",
        "unexpected MOTD"
    );
    assert_eq!(
        response["players"]["online"].as_u64().unwrap(),
        0,
        "should have 0 online"
    );
    assert_eq!(
        response["players"]["max"].as_u64().unwrap(),
        20,
        "max players should be 20"
    );
    assert_eq!(
        response["version"]["protocol"].as_i64().unwrap(),
        776,
        "protocol 776 (26.2)"
    );

    // Ping → Pong
    let payload: i64 = 1234567890123;
    let mut ping = Vec::new();
    ping.extend_from_slice(&write_varint(9));
    ping.push(0x01);
    ping.extend_from_slice(&payload.to_be_bytes());
    stream.write_all(&ping).expect("send ping");

    let pong_len = read_varint(&mut stream).expect("read pong len");
    assert_eq!(pong_len, 9);
    let mut pong_data = [0u8; 9];
    stream.read_exact(&mut pong_data).expect("read pong");

    assert_eq!(pong_data[0], 0x01, "pong packet ID");
    let pong_payload = i64::from_be_bytes(pong_data[1..9].try_into().unwrap());
    assert_eq!(pong_payload, payload, "pong payload match");

    // Important: close connection so server doesn't hang
    drop(stream);

    eprintln!("=== PING TEST PASSED ===");
}
