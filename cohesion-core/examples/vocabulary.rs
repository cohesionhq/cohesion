//! Prints the `Vocabulary` a `Hello` carries, as the web client's mock and
//! tests read it: `web/src/lib/protocol/vocabulary.json`.

fn main() {
    let vocabulary = cohesion_core::protocol::Vocabulary::new();
    print!("{}", serde_json::to_string(&vocabulary).unwrap());
}
