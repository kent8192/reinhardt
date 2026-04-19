fn main() {
    use reinhardt_query::nosql::redis::zset::ZSetCommand;
    let _ = ZSetCommand::zadd("z").only_if_greater().only_if_less();
}
