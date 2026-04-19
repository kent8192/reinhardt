//! Redis RESP3 command output type.

/// A Redis command represented as a RESP3 array of bulk strings.
///
/// Created by command builders via [`CommandStatementBuilder::build`](super::command::CommandStatementBuilder::build).
/// Use [`to_resp3_bytes`](RespCommand::to_resp3_bytes) to obtain wire-format bytes
/// suitable for sending directly over a TCP connection to Redis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RespCommand {
    args: Vec<Vec<u8>>,
}

impl RespCommand {
    pub(crate) fn new(args: Vec<Vec<u8>>) -> Self {
        Self { args }
    }

    /// Returns the command arguments as a slice of byte vectors.
    pub fn args(&self) -> &[Vec<u8>] {
        &self.args
    }

    /// Consumes the command and returns the inner argument vector.
    pub fn into_args(self) -> Vec<Vec<u8>> {
        self.args
    }

    /// Serializes the command to RESP3 wire format.
    ///
    /// Format: `*N\r\n` followed by N bulk strings (`$len\r\ndata\r\n`).
    pub fn to_resp3_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(format!("*{}\r\n", self.args.len()).as_bytes());
        for arg in &self.args {
            buf.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
            buf.extend_from_slice(arg);
            buf.extend_from_slice(b"\r\n");
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[rstest]
    fn test_resp3_bytes_set() {
        // Arrange
        let cmd = RespCommand::new(vec![
            b"SET".to_vec(),
            b"mykey".to_vec(),
            b"myvalue".to_vec(),
        ]);

        // Act
        let bytes = cmd.to_resp3_bytes();

        // Assert
        assert_eq!(bytes, b"*3\r\n$3\r\nSET\r\n$5\r\nmykey\r\n$7\r\nmyvalue\r\n");
    }

    #[rstest]
    fn test_args_returns_slice() {
        // Arrange
        let cmd = RespCommand::new(vec![b"PING".to_vec()]);

        // Assert
        assert_eq!(cmd.args(), &[b"PING".to_vec()]);
    }

    #[rstest]
    fn test_into_args_consumes() {
        // Arrange
        let cmd = RespCommand::new(vec![b"GET".to_vec(), b"k".to_vec()]);

        // Act
        let args = cmd.into_args();

        // Assert
        assert_eq!(args, vec![b"GET".to_vec(), b"k".to_vec()]);
    }

    #[rstest]
    fn test_resp3_bytes_single_arg() {
        // Arrange
        let cmd = RespCommand::new(vec![b"PING".to_vec()]);

        // Act
        let bytes = cmd.to_resp3_bytes();

        // Assert
        assert_eq!(bytes, b"*1\r\n$4\r\nPING\r\n");
    }
}
