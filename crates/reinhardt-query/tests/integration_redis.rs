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

    async fn connect(port: u16) -> TcpStream {
        TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .expect("failed to connect to Redis")
    }

    async fn send(stream: &mut TcpStream, cmd: &RespCommand) {
        stream.write_all(&cmd.to_resp3_bytes()).await.unwrap();
    }

    async fn send_recv(stream: &mut TcpStream, cmd: &RespCommand) -> Vec<u8> {
        stream.write_all(&cmd.to_resp3_bytes()).await.unwrap();
        let mut buf = vec![0u8; 4096];
        let n = stream.read(&mut buf).await.unwrap();
        buf[..n].to_vec()
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

        // Read all responses (MULTI/EXEC produce multiple lines)
        let mut buf = vec![0u8; 4096];
        let _ = conn.read(&mut buf).await.unwrap();

        // Verify final state
        let get_resp = send_recv(&mut conn, &StringCommand::get("tx_key").build()).await;

        // Assert
        assert_eq!(parse_bulk_string(&get_resp), Some(b"tx_value".to_vec()));
    }
}
