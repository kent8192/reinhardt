fn main() {
    use reinhardt_query::nosql::redis::string::StringCommand;
    let _ = StringCommand::set("k", "v").ex(60).px(100);
}
