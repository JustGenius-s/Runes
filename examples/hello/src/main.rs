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

    let markers = runes_runtime::take_root_markers();
    assert_eq!(markers.len(), 1);
    println!("native rune root: {}", markers[0].site.binding);
}
