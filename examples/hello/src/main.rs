#[derive(Debug)]
struct User {
    name: String,
}

fn normalize_name(value: &str) -> String {
    value.trim().to_uppercase()
}

fn main() {
    rune user = User {
        name: "  Ada Lovelace  ".to_owned(),
    };

    let normalized = normalize_name(&user.name);
    println!("Hello, {normalized}");

    if let Err(error) = runes_runtime::write_trace_file_default() {
        eprintln!("failed to write trace: {error}");
    }
}
