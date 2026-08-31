//! Runes demo: every supported root value shape plus multi-root union.
//!
//! The single `rune` marker works across scalars, structs, enums, containers,
//! references, closures and `Box<dyn Trait>`. Later roots flow into shared
//! computation so the trace shows multi-root provenance (`roots=[...]`).

use std::collections::HashMap;

#[derive(Debug, Clone)]
struct User {
    name: String,
    score: u32,
}

#[derive(Debug, Clone)]
enum Status {
    Active,
    Paused,
}

trait Render {
    fn render(&self) -> String;
}

impl Render for User {
    fn render(&self) -> String {
        format!("{} (score {})", self.name, self.score)
    }
}

/// Scalar helpers that exercise call tracing inside a callee.
fn double(value: i32) -> i32 {
    value * 2
}

fn describe_user(user: &User) -> String {
    user.name.trim().to_uppercase()
}

fn sum_scores(users: &[User]) -> u32 {
    users.iter().map(|u| u.score).sum()
}

fn main() {
    // --- scalars ---
    rune count = 21_i32;
    let doubled = double(count);

    // --- struct ---
    rune user = User {
        name: "  Ada Lovelace  ".to_owned(),
        score: 99,
    };

    // --- enum ---
    rune status = Status::Active;
    let _paused = Status::Paused; // exercise the second variant

    // --- containers ---
    rune tags = vec!["math".to_owned(), "pioneer".to_owned()];
    rune scores = HashMap::from([("ada", 99_u32), ("grace", 88_u32)]);

    // Consume the containers through traced calls so their provenance shows up
    // (macro-expansion internals like `println!` are deliberately not traced).
    let tag_count = tags.len();
    let ada_score = scores.get("ada").copied().unwrap_or(0);

    // --- reference (borrow of a traced root) ---
    rune user_ref = &user;
    let name = describe_user(user_ref);

    // --- closure ---
    rune bump = |v: i32| v + 1;
    let bumped = bump(count);

    // --- Box<dyn Trait> ---
    rune renderable = Box::new(user.clone()) as Box<dyn Render>;
    let rendered = renderable.render();

    // --- multi-root union: many roots flow into one computation ---
    let others = vec![
        User { name: "grace hopper".to_owned(), score: 88 },
        User { name: "katherine johnson".to_owned(), score: 95 },
    ];
    let everyone = vec![user.clone(), others[0].clone(), others[1].clone()];
    let total = sum_scores(&everyone);

    let status_label = match status {
        Status::Active => "active",
        Status::Paused => "paused",
    };

    println!("doubled: {doubled}");
    println!("name: {name}");
    println!("tags: {tags:?} (count {tag_count})");
    println!("scores: {scores:?} (ada {ada_score})");
    println!("bumped: {bumped}");
    println!("rendered: {rendered}");
    println!("total score: {total} ({status_label})");

    if let Err(error) = runes_runtime::write_trace_file_default() {
        eprintln!("failed to write trace: {error}");
    }
}
