//! Redis transaction and scripting command builders.

use super::{
    command::CommandStatementBuilder,
    key::{IntoRedisKey, RedisKey},
    resp::RespCommand,
    value::ToRedisBytes,
};

/// Wraps a sequence of commands in a Redis MULTI/EXEC transaction.
#[derive(Debug, Default)]
pub struct Transaction {
    commands: Vec<RespCommand>,
}

impl Transaction {
    /// Create a new empty transaction.
    pub fn new() -> Self {
        Self { commands: Vec::new() }
    }

    /// Add a command to the transaction.
    pub fn command(mut self, cmd: impl CommandStatementBuilder) -> Self {
        self.commands.push(cmd.build());
        self
    }

    /// Build the transaction into `[MULTI, cmd1, cmd2, ..., EXEC]`.
    pub fn build(self) -> Vec<RespCommand> {
        let mut result = Vec::with_capacity(self.commands.len() + 2);
        result.push(RespCommand::new(vec![b"MULTI".to_vec()]));
        result.extend(self.commands);
        result.push(RespCommand::new(vec![b"EXEC".to_vec()]));
        result
    }
}

/// Entry point for Redis scripting commands.
pub struct ScriptCommand;

impl ScriptCommand {
    /// Build an EVAL command.
    pub fn eval(script: impl Into<String>) -> EvalBuilder {
        EvalBuilder {
            script_or_sha: script.into(),
            is_sha: false,
            keys: Vec::new(),
            args: Vec::new(),
        }
    }

    /// Build an EVALSHA command.
    pub fn evalsha(sha: impl Into<String>) -> EvalBuilder {
        EvalBuilder {
            script_or_sha: sha.into(),
            is_sha: true,
            keys: Vec::new(),
            args: Vec::new(),
        }
    }
}

/// Builder for EVAL/EVALSHA commands.
#[derive(Debug)]
pub struct EvalBuilder {
    script_or_sha: String,
    is_sha: bool,
    keys: Vec<RedisKey>,
    args: Vec<Vec<u8>>,
}

impl EvalBuilder {
    /// Add a key argument.
    pub fn key(mut self, k: impl IntoRedisKey) -> Self {
        self.keys.push(k.into_redis_key());
        self
    }

    /// Add a non-key argument.
    pub fn arg(mut self, v: impl ToRedisBytes) -> Self {
        self.args.push(v.to_redis_bytes());
        self
    }
}

impl CommandStatementBuilder for EvalBuilder {
    fn build(&self) -> RespCommand {
        let cmd = if self.is_sha { b"EVALSHA".to_vec() } else { b"EVAL".to_vec() };
        let mut args = vec![
            cmd,
            self.script_or_sha.as_bytes().to_vec(),
            self.keys.len().to_string().into_bytes(),
        ];
        args.extend(self.keys.iter().map(|k| k.to_bytes()));
        args.extend(self.args.iter().cloned());
        RespCommand::new(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nosql::redis::string::StringCommand;
    use rstest::*;

    #[rstest]
    fn test_transaction_wraps_with_multi_exec() {
        // Arrange
        let tx = Transaction::new()
            .command(StringCommand::set("k", "v").ex(10))
            .command(StringCommand::incr("counter"));

        // Act
        let cmds = tx.build();

        // Assert
        assert_eq!(cmds.len(), 4);
        assert_eq!(cmds[0].args(), &[b"MULTI".to_vec()]);
        assert_eq!(cmds[1].args(), &[
            b"SET".to_vec(), b"k".to_vec(), b"v".to_vec(),
            b"EX".to_vec(), b"10".to_vec(),
        ]);
        assert_eq!(cmds[2].args(), &[b"INCR".to_vec(), b"counter".to_vec()]);
        assert_eq!(cmds[3].args(), &[b"EXEC".to_vec()]);
    }

    #[rstest]
    fn test_eval_command() {
        // Arrange + Act
        let cmd = ScriptCommand::eval("return 1").key("k1").arg("arg1").build();

        // Assert
        assert_eq!(cmd.args(), &[
            b"EVAL".to_vec(), b"return 1".to_vec(), b"1".to_vec(),
            b"k1".to_vec(), b"arg1".to_vec(),
        ]);
    }

    #[rstest]
    fn test_evalsha_command() {
        // Arrange + Act
        let cmd = ScriptCommand::evalsha("abc123").build();

        // Assert
        assert_eq!(cmd.args(), &[
            b"EVALSHA".to_vec(), b"abc123".to_vec(), b"0".to_vec(),
        ]);
    }

    #[rstest]
    fn test_empty_transaction() {
        // Arrange + Act
        let cmds = Transaction::new().build();

        // Assert -- even empty transaction has MULTI and EXEC
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[0].args(), &[b"MULTI".to_vec()]);
        assert_eq!(cmds[1].args(), &[b"EXEC".to_vec()]);
    }
}
