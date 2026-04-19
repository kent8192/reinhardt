fn main() {
    use reinhardt_query::nosql::redis::zset::ZSetCommand;
    let _ = ZSetCommand::zadd("z").nx().only_if_greater();
}
