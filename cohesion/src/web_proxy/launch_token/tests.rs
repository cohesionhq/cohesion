use std::fs;

use super::saved;
use crate::client_storage::StoragePath;

#[test]
fn the_token_survives_launches_until_reset() {
    let dir = std::env::temp_dir().join(format!("cohesion-launch-token-{}", std::process::id()));
    let storage = StoragePath::resolve(dir.to_str().unwrap()).unwrap();

    let first = saved(&storage, false).unwrap();
    assert_eq!(saved(&storage, false).unwrap(), first);
    let reset = saved(&storage, true).unwrap();
    assert_ne!(reset, first);
    assert_eq!(saved(&storage, false).unwrap(), reset);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_emptied_or_partial_token_file_is_replaced() {
    let dir = std::env::temp_dir().join(format!(
        "cohesion-launch-token-partial-{}",
        std::process::id()
    ));
    let storage = StoragePath::resolve(dir.to_str().unwrap()).unwrap();

    for damaged in ["", "\n", "abc"] {
        fs::write(storage.web_token_file(), damaged).unwrap();
        let token = saved(&storage, false).unwrap();
        assert_eq!(token.len(), 32, "{damaged:?} was served as the token");
        assert_eq!(saved(&storage, false).unwrap(), token);
    }

    fs::remove_dir_all(dir).unwrap();
}
