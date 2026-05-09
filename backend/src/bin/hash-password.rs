fn main() {
    let password = std::env::args()
        .nth(1)
        .expect("usage: cargo run --bin hash-password -- <password>");
    let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).expect("hash password");
    println!("{hash}");
}
