//! Which methods the facade really has, read from its own sources.
//!
//! Its own file because it is the one piece of the parity gate that
//! parses Rust rather than comparing lists, and because the file it
//! scans is being split (#67): keeping the scan self-contained means
//! that change and this one meet in one place instead of three.

use std::collections::BTreeSet;

/// Where the facade lives. **The directory**, not one file in it.
///
/// Parsed rather than mirrored in a const list, so that adding a method is
/// enough to break this test — a mirror has to be updated to break. Read at
/// test time rather than with `include_str!` for the same reason one step
/// further out: `include_str!` names a file, and a file named in a test is a
/// file the test stops seeing the day somebody splits it. `embedded_made.rs`
/// is being split right now (#67), and the scan below has to find the halves
/// without anyone remembering to tell it.
pub(super) const EMBEDDED_MADE_SOURCE_DIR: &str = "../made-embedded/src";

/// Every public method of `EmbeddedMade`, wherever it is declared.
///
/// Two things this used to miss, both of them the kind of gap a gate is
/// supposed to be immune to: it read one file, so a second `impl` block in a
/// second file was invisible; and it matched two spellings, `pub fn ` and
/// `pub async fn `, so `pub const fn version` was invisible in the file it
/// did read. Neither failure announces itself — the test stays green and
/// stops covering something.
///
/// Now: every `.rs` file under the facade's crate, every `impl EmbeddedMade`
/// block in it, and every form of `pub fn` inside one — `pub fn`, `pub async
/// fn`, `pub const fn`, and any order of those modifiers. `pub(crate)` and
/// `pub(super)` are not public and are left out, which is why the prefix
/// tested is `pub ` with its space.
pub(super) fn embedded_made_public_methods() -> BTreeSet<String> {
    let mut methods = BTreeSet::new();
    let mut blocks = 0_usize;
    for source in facade_sources() {
        for block in impl_blocks(&source, "EmbeddedMade") {
            blocks += 1;
            methods.extend(public_fn_names(block));
        }
    }
    assert!(
        blocks > 0,
        "no `impl EmbeddedMade` block was found under {EMBEDDED_MADE_SOURCE_DIR}; \
         the scan is reading the wrong place, and a scan that finds nothing \
         agrees with every exception file there is"
    );
    methods
}

/// The text of every Rust source of the facade's crate.
fn facade_sources() -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EMBEDDED_MADE_SOURCE_DIR);
    let mut sources = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("{} cannot be read: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                sources.push(
                    std::fs::read_to_string(&path).unwrap_or_else(|error| {
                        panic!("{} cannot be read: {error}", path.display())
                    }),
                );
            }
        }
    }
    assert!(
        !sources.is_empty(),
        "no Rust source was found under {}",
        root.display()
    );
    sources
}

/// The lines inside each `impl <type> {` block of one source.
///
/// A block ends at the first line that is a lone `}` in column zero, which
/// is what rustfmt writes and what the formatting gate enforces. Trait impls
/// (`impl Trait for <type>`) are not this type's own surface and do not
/// match: the line has to be `impl <type> {` exactly.
fn impl_blocks<'a>(source: &'a str, type_name: &str) -> Vec<Vec<&'a str>> {
    let opening = format!("impl {type_name} {{");
    let mut blocks = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in source.lines() {
        match current.as_mut() {
            None => {
                if line.trim() == opening {
                    current = Some(Vec::new());
                }
            }
            Some(block) => {
                if line == "}" {
                    blocks.push(current.take().expect("a block is open"));
                } else {
                    block.push(line);
                }
            }
        }
    }
    blocks
}

/// Every `pub fn` in one block, in whatever order its modifiers are written.
fn public_fn_names(block: Vec<&str>) -> BTreeSet<String> {
    block
        .into_iter()
        .filter_map(|line| {
            // `pub ` with the space: `pub(crate)` and `pub(super)` are not
            // public surface and must not be counted as capabilities.
            let mut rest = line.trim().strip_prefix("pub ")?;
            loop {
                if let Some(after) = rest.strip_prefix("fn ") {
                    return after.split(['(', '<', ' ']).next().map(str::to_owned);
                }
                rest = ["async ", "const ", "unsafe ", "extern "]
                    .iter()
                    .find_map(|modifier| rest.strip_prefix(modifier))?;
            }
        })
        .collect()
}
