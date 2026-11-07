//! The names an agent gets when it is created without one.

/// Co-worker-style first names, in the order they are handed out.
const NAMES: [&str; 48] = [
    "Ada", "Milo", "Iris", "Theo", "Nora", "Felix", "Hazel", "Otto", "June", "Arlo", "Mabel",
    "Silas", "Ruth", "Jasper", "Wren", "Hugo", "Clara", "Ezra", "Ivy", "Rufus", "Edith", "Leo",
    "Opal", "Gus", "Vera", "Basil", "Pearl", "Cyrus", "Maude", "Emil", "Nell", "Ansel", "Greta",
    "Otis", "Fern", "Abel", "Lena", "Vito", "Rosa", "Kit", "Dora", "Bram", "Tess", "Ike", "Lyra",
    "Amos", "Bea", "Sol",
];

/// A name none of `taken` uses (ignoring case), starting at a position
/// `seed` picks so consecutive agents don't always get the same one. Once
/// every name is taken, names repeat with a number: `Ada 2`.
pub(super) fn agent_name(taken: &[String], seed: i64) -> String {
    let start = seed.rem_euclid(NAMES.len() as i64) as usize;
    (1..)
        .flat_map(|round| {
            (0..NAMES.len()).map(move |i| {
                let name = NAMES[(start + i) % NAMES.len()];
                if round == 1 {
                    name.to_string()
                } else {
                    format!("{name} {round}")
                }
            })
        })
        .find(|name| !taken.iter().any(|t| t.eq_ignore_ascii_case(name)))
        .unwrap()
}
