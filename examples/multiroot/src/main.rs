//! Multi-root union and in-callee inheritance demo.
//!
//! Two `rune` roots (`user` and `order`) flow into a single call to `combine`,
//! which then makes traced calls on both inputs. Every event should carry the
//! full root set `roots=[order, user]` once the two roots merge.

fn combine(a: &str, b: &str) -> String {
    let x = a.to_uppercase();
    let y = b.to_uppercase();
    format!("{x}-{y}")
}

fn main() {
    rune user = String::from("Ada");
    rune order = String::from("42");

    let merged = combine(&user, &order);
    println!("{merged}");

    if let Err(error) = runes_runtime::write_trace_file_default() {
        eprintln!("failed to write trace: {error}");
    }
}
