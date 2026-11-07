//! Prints the web client's protocol types: `web/src/lib/protocol/generated.ts`.

fn main() {
    print!("{}", cohesion_core::ts_wire::client_module());
}
