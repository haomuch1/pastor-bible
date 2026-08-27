pub fn sqlite_version() -> String {
    let c = rusqlite::Connection::open_in_memory().unwrap();
    c.query_row("select sqlite_version()", [], |r| r.get::<_, String>(0)).unwrap()
}
pub fn has_fts5() -> bool {
    let c = rusqlite::Connection::open_in_memory().unwrap();
    c.execute_batch("create virtual table t using fts5(x)").is_ok()
}
