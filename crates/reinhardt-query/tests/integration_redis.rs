#[cfg(feature = "nosql-redis")]
mod redis_integration {
    use reinhardt_query::nosql::redis::{
        command::CommandStatementBuilder,
        resp::RespCommand,
        string::StringCommand,
        transaction::Transaction,
        zset::ZSetCommand,
    };
    use rstest::*;
    use testcontainers_modules::redis::Redis;
    use testcontainers::runners::AsyncRunner;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::{Duration, timeout};

    async fn connect(port: u16) -> TcpStream {
        // Retry with exponential backoff while Redis is starting up inside the container.
        let mut retries = 0u32;
        loop {
            match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
                Ok(stream) => return stream,
                Err(_) if retries < 8 => {
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(retries))).await;
                }
                Err(e) => panic!("failed to connect to Redis after {} retries: {}", retries, e),
            }
        }
    }

    async fn send(stream: &mut TcpStream, cmd: &RespCommand) {
        stream.write_all(&cmd.to_resp3_bytes()).await.unwrap();
    }

    async fn send_recv(stream: &mut TcpStream, cmd: &RespCommand) -> Vec<u8> {
        stream.write_all(&cmd.to_resp3_bytes()).await.unwrap();
        read_until_complete(stream).await
    }

    /// Read RESP frames from the stream until at least one complete frame is
    /// buffered. TCP reads may return partial data, so a single `read()` call
    /// is not sufficient. This loops until the accumulated buffer contains a
    /// complete frame (or no more data arrives within a short timeout).
    async fn read_until_complete(stream: &mut TcpStream) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::with_capacity(4096);
        let mut tmp = [0u8; 4096];
        loop {
            match timeout(Duration::from_millis(500), stream.read(&mut tmp)).await {
                Ok(Ok(0)) => break,
                Ok(Ok(n)) => {
                    buf.extend_from_slice(&tmp[..n]);
                    if resp_frame_complete(&buf) {
                        break;
                    }
                }
                Ok(Err(e)) => panic!("read error: {}", e),
                // Timed out waiting for more data; return what we have.
                Err(_) => break,
            }
        }
        buf
    }

    /// Check if the buffer contains at least one complete RESP2/RESP3 frame.
    /// Returns the total consumed length on success, or `None` if incomplete.
    fn resp_frame_len(buf: &[u8]) -> Option<usize> {
        if buf.is_empty() {
            return None;
        }
        match buf[0] {
            // Simple string / error / integer / null / boolean / big-number:
            // terminated by the first CRLF.
            b'+' | b'-' | b':' | b'_' | b'#' | b'(' => {
                let rel = find_crlf(&buf[1..])?;
                Some(1 + rel + 2)
            }
            // Bulk string: $<len>\r\n<payload>\r\n (len == -1 means null bulk).
            b'$' => {
                let rel = find_crlf(&buf[1..])?;
                let header_end = 1 + rel + 2;
                let len: i64 = std::str::from_utf8(&buf[1..1 + rel]).ok()?.parse().ok()?;
                if len < 0 {
                    return Some(header_end);
                }
                let total = header_end + len as usize + 2;
                if buf.len() >= total { Some(total) } else { None }
            }
            // Array / map / set: aggregate types contain N child frames.
            b'*' | b'~' => {
                let rel = find_crlf(&buf[1..])?;
                let mut cursor = 1 + rel + 2;
                let count: i64 = std::str::from_utf8(&buf[1..1 + rel]).ok()?.parse().ok()?;
                if count < 0 {
                    return Some(cursor);
                }
                for _ in 0..count {
                    let child_len = resp_frame_len(&buf[cursor..])?;
                    cursor += child_len;
                }
                Some(cursor)
            }
            b'%' => {
                let rel = find_crlf(&buf[1..])?;
                let mut cursor = 1 + rel + 2;
                let count: i64 = std::str::from_utf8(&buf[1..1 + rel]).ok()?.parse().ok()?;
                if count < 0 {
                    return Some(cursor);
                }
                // Maps have 2 * count child frames (key + value pairs).
                for _ in 0..count * 2 {
                    let child_len = resp_frame_len(&buf[cursor..])?;
                    cursor += child_len;
                }
                Some(cursor)
            }
            // Unknown prefix: fall back to the next CRLF so the loop can stop.
            _ => find_crlf(buf).map(|r| r + 2),
        }
    }

    fn resp_frame_complete(buf: &[u8]) -> bool {
        resp_frame_len(buf).is_some()
    }

    fn find_crlf(buf: &[u8]) -> Option<usize> {
        buf.windows(2).position(|w| w == b"\r\n")
    }

    fn parse_bulk_string(resp: &[u8]) -> Option<Vec<u8>> {
        if !resp.starts_with(b"$") {
            return None;
        }
        let crlf = resp.iter().position(|&b| b == b'\r')?;
        let len: usize = std::str::from_utf8(&resp[1..crlf]).ok()?.parse().ok()?;
        let start = crlf + 2;
        if resp.len() >= start + len {
            Some(resp[start..start + len].to_vec())
        } else {
            None
        }
    }

    fn is_ok(resp: &[u8]) -> bool {
        resp.starts_with(b"+OK\r\n")
    }

    fn parse_integer(resp: &[u8]) -> Option<i64> {
        if !resp.starts_with(b":") {
            return None;
        }
        let crlf = resp.iter().position(|&b| b == b'\r')?;
        std::str::from_utf8(&resp[1..crlf]).ok()?.parse().ok()
    }

    #[rstest]
    #[tokio::test]
    async fn test_set_get_roundtrip() {
        // Arrange
        let container = Redis::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        let mut conn = connect(port).await;

        // Act
        let set_resp = send_recv(&mut conn, &StringCommand::set("hello", "world").ex(60).build()).await;
        let get_resp = send_recv(&mut conn, &StringCommand::get("hello").build()).await;

        // Assert
        assert!(is_ok(&set_resp), "SET should return +OK, got: {:?}", set_resp);
        assert_eq!(parse_bulk_string(&get_resp), Some(b"world".to_vec()));
    }

    #[rstest]
    #[tokio::test]
    async fn test_incr_roundtrip() {
        // Arrange
        let container = Redis::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        let mut conn = connect(port).await;

        // Act
        send_recv(&mut conn, &StringCommand::set("counter", "10").build()).await;
        let resp = send_recv(&mut conn, &StringCommand::incr("counter").build()).await;

        // Assert
        assert_eq!(parse_integer(&resp), Some(11));
    }

    #[rstest]
    #[tokio::test]
    async fn test_set_nx_does_not_overwrite() {
        // Arrange
        let container = Redis::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        let mut conn = connect(port).await;

        // Act
        send_recv(&mut conn, &StringCommand::set("k", "original").build()).await;
        let nx_resp = send_recv(&mut conn, &StringCommand::set("k", "new").nx().build()).await;
        let get_resp = send_recv(&mut conn, &StringCommand::get("k").build()).await;

        // Assert
        // NX on existing key: Redis returns nil bulk string ($-1\r\n) or null
        assert!(
            nx_resp.starts_with(b"$-1") || nx_resp.starts_with(b"_"),
            "NX on existing key should return nil, got: {:?}",
            nx_resp
        );
        assert_eq!(parse_bulk_string(&get_resp), Some(b"original".to_vec()));
    }

    #[rstest]
    #[tokio::test]
    async fn test_zadd_zrange_roundtrip() {
        // Arrange
        let container = Redis::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        let mut conn = connect(port).await;

        // Act
        let zadd = ZSetCommand::zadd("leaderboard")
            .member(100.0, "alice")
            .member(200.0, "bob")
            .build();
        send(&mut conn, &zadd).await;
        let zrange_resp = send_recv(&mut conn, &ZSetCommand::zrange("leaderboard", 0, -1).build()).await;

        // Assert
        let resp_str = String::from_utf8_lossy(&zrange_resp);
        assert!(resp_str.contains("alice"), "ZRANGE should include alice");
        assert!(resp_str.contains("bob"), "ZRANGE should include bob");
    }

    #[rstest]
    #[tokio::test]
    async fn test_transaction_multi_exec() {
        // Arrange
        let container = Redis::default().start().await.unwrap();
        let port = container.get_host_port_ipv4(6379).await.unwrap();
        let mut conn = connect(port).await;

        // Act
        let cmds = Transaction::new()
            .command(StringCommand::set("tx_key", "tx_value"))
            .command(StringCommand::incr("tx_counter"))
            .build();

        let mut all_bytes = Vec::new();
        for cmd in &cmds {
            all_bytes.extend_from_slice(&cmd.to_resp3_bytes());
        }
        conn.write_all(&all_bytes).await.unwrap();

        // Read all responses (MULTI/EXEC produce multiple lines). Loop until
        // no more data arrives so partial TCP reads don't leave responses in
        // the kernel buffer for the next test call.
        let _ = read_until_complete(&mut conn).await;

        // Verify final state
        let get_resp = send_recv(&mut conn, &StringCommand::get("tx_key").build()).await;

        // Assert
        assert_eq!(parse_bulk_string(&get_resp), Some(b"tx_value".to_vec()));
    }
}
