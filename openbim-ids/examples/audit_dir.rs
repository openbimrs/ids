//! Prints audit findings for every `.ids` file under a directory.
//!
//! `cargo run --features audit --example audit_dir -- <dir>`

fn main() {
    let dir = std::env::args().nth(1).expect("usage: audit_dir <dir>");
    let mut stack = vec![std::path::PathBuf::from(dir)];
    let mut files = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable directory") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "ids") {
                files.push(path);
            }
        }
    }
    files.sort();
    for file in files {
        let bytes = std::fs::read(&file).expect("readable file");
        match openbim_ids::from_slice(&bytes) {
            Ok(ids) => {
                for finding in openbim_ids::audit(&ids) {
                    println!("{}: {finding}", file.display());
                }
            }
            Err(error) => println!("{}: read error: {error}", file.display()),
        }
    }
}
