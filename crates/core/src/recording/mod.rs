//! The recording as the analysers see it (port spec 3.2): an in-memory tree of a recording directory or of the
//! whole evidence directory, ordered as Python sorts `Path` objects, whose `list` and `glob` never return a folder
//! named `golden` (spec section 5 reserves the name); the decoders and line splitters of the Python's readers; the
//! Python spellings of strings and numbers; `Ordered`; and the recorder's file types (`files`). No I/O: `crates/dev`
//! fills the tree from disk.

pub mod files;
mod ordered;
mod printable;
mod python;
mod text;

pub use ordered::Ordered;
pub use python::{
    py_fixed, py_float, py_float_repr, py_float_value, py_g, py_int, py_int_value, py_is_space, py_isdigit, py_repr,
    py_repr_str, py_split, py_str, py_strip, py_truthy,
};
pub use text::{
    decode_text, py_bytes_splitlines, py_splitlines, strip_cr_lines, text_mode, universal_lines, Encoding, Errors,
};

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Bound;
use std::sync::{Arc, OnceLock};

use sha1::{Digest, Sha1};

/// The SHA-1 of `bytes` as 40 lowercase hex digits (Python's `hashlib.sha1(bytes).hexdigest()`).
pub fn sha1_hex(bytes: &[u8]) -> String {
    Sha1::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The folder name the port spec (section 5) reserves for goldens: `list` and `glob` never return one, nor anything
/// under one.
const GOLDEN: &str = "golden";

/// A path inside a tree: '/'-separated components below the tree's root. Ordered component by component, each
/// name by code point (derived `Ord` over `Vec<String>`), which is how Python sorts `PurePosixPath` objects:
/// `all-tracks/x` < `all-tracks-2/x` and `audio-levels-20ms-1-2/x` < `audio-levels-20ms-1-2-3/x`.
/// Py: pathlib.PurePosixPath ordering.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PathKey(Vec<String>);

impl PathKey {
    /// The tree's root (no component).
    pub fn root() -> PathKey {
        PathKey(Vec::new())
    }

    /// A key from "a/b/c"; empty and "." components are dropped, as pathlib drops them.
    pub fn new(path: &str) -> PathKey {
        PathKey(path.split('/').filter(|part| !part.is_empty() && *part != ".").map(str::to_string).collect())
    }

    /// This key with `name` appended ("x/y" appends two components). Py: `path / name`.
    pub fn join(&self, name: &str) -> PathKey {
        let mut parts = self.0.clone();
        parts.extend(PathKey::new(name).0);
        PathKey(parts)
    }

    /// The key without its last component (the root's parent is the root). Py: Path.parent.
    pub fn parent(&self) -> PathKey {
        let mut parts = self.0.clone();
        parts.pop();
        PathKey(parts)
    }

    /// The last component, "" for the root. Py: Path.name.
    pub fn name(&self) -> &str {
        self.0.last().map(String::as_str).unwrap_or("")
    }

    /// The components in order.
    pub fn components(&self) -> &[String] {
        &self.0
    }

    /// Whether this key lies at or below `prefix`.
    fn starts_with(&self, prefix: &PathKey) -> bool {
        self.0.len() >= prefix.0.len() && self.0[..prefix.0.len()] == prefix.0[..]
    }
}

/// '/'-joined components, "" for the root.
impl fmt::Display for PathKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0.join("/"))
    }
}

/// What a path names. Py: Path.is_file / Path.is_dir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Dir,
}

/// A file the tree does not have. Its Display is Python's FileNotFoundError text,
/// `[Errno 2] No such file or directory: '<shown>'`, with `shown` the path as `Tree::display` prints it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    pub path: PathKey,
    pub shown: String,
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[Errno 2] No such file or directory: {}", py_repr_str(&self.shown))
    }
}

/// Why a read failed: the file is not there, or the OS refused it (the message is the OSError text).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadError {
    Missing(Missing),
    Io { path: PathKey, message: String },
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadError::Missing(missing) => fmt::Display::fmt(missing, f),
            ReadError::Io { message, .. } => f.write_str(message),
        }
    }
}

/// A recording directory or the whole evidence directory, in memory. Every walker of the Python is a function over
/// it. `list` and `glob` never return a folder named `golden` or anything under one; `read`, `kind` and `exists` do
/// not filter, so the golden code reads its files through the same tree. Py: the pathlib calls of every module.
pub trait Tree {
    /// File, folder, or None when absent; the root is always a folder. Py: Path.exists / is_file / is_dir.
    fn kind(&self, path: &PathKey) -> Option<Kind>;

    /// `self.kind(path).is_some()`. Py: Path.exists.
    fn exists(&self, path: &PathKey) -> bool {
        self.kind(path).is_some()
    }

    /// Py: Path.is_file.
    fn is_file(&self, path: &PathKey) -> bool {
        self.kind(path) == Some(Kind::File)
    }

    /// Py: Path.is_dir.
    fn is_dir(&self, path: &PathKey) -> bool {
        self.kind(path) == Some(Kind::Dir)
    }

    /// The entries directly in `dir`, files and folders, in `PathKey` order; empty for a file or an absent path.
    /// Py: sorted(dir.iterdir()).
    fn list(&self, dir: &PathKey) -> Vec<PathKey>;

    /// The paths below `base` matching `pattern`: '/'-separated, each component an fnmatch pattern (`*`, `?`,
    /// `[seq]`, `[!seq]`, case-sensitive, a leading dot matched like any character, as pathlib's glob), every
    /// component but the last naming a folder, in `PathKey` order. Py: sorted(base.glob(pattern)).
    fn glob(&self, base: &PathKey, pattern: &str) -> Vec<PathKey>;

    /// The file's bytes; a lazy entry is loaded on its first read and kept. Py: Path.read_bytes.
    fn read(&self, path: &PathKey) -> Result<Arc<[u8]>, ReadError>;

    /// The path as the Python printed it: the tree's display root joined with `path` as pathlib joins (a display
    /// root of "." adds nothing). Py: str(Path).
    fn display(&self, path: &PathKey) -> String;

    /// The last component of the path's resolved absolute form; for the root, the resolved root's own name.
    /// Py: Path.resolve().name.
    fn resolved_name(&self, path: &PathKey) -> String;
}

/// Loads a lazy file's bytes once; `Err` holds the OSError text.
pub type Loader = Arc<dyn Fn() -> Result<Vec<u8>, String> + Send + Sync>;

enum Node {
    Dir,
    File(Arc<[u8]>),
    Lazy(Loader, OnceLock<Arc<[u8]>>),
}

/// The in-memory `Tree`: a `BTreeMap<PathKey, Node>` with folders explicit (an empty folder exists, as on disk),
/// files as bytes or as a `Loader`. Tests build it directly; `crates/dev` fills it from a directory.
pub struct MemTree {
    entries: BTreeMap<PathKey, Node>,
    display_root: String,
    resolved_root: String,
}

impl MemTree {
    /// An empty tree printed under `display_root` ("corpus/owner", ".", or a path as the user gave it) whose root
    /// resolves to a folder named `resolved_root_name`.
    pub fn new(display_root: &str, resolved_root_name: &str) -> MemTree {
        MemTree {
            entries: BTreeMap::new(),
            display_root: pathlib_str(display_root),
            resolved_root: resolved_root_name.to_string(),
        }
    }

    /// Adds a file (its parent folders too).
    pub fn insert_file(&mut self, path: &str, bytes: impl Into<Vec<u8>>) {
        let key = PathKey::new(path);
        self.insert_parents(&key);
        let bytes: Vec<u8> = bytes.into();
        self.entries.insert(key, Node::File(Arc::from(bytes)));
    }

    /// Adds a file read on first use.
    pub fn insert_lazy(&mut self, path: &str, load: Loader) {
        let key = PathKey::new(path);
        self.insert_parents(&key);
        self.entries.insert(key, Node::Lazy(load, OnceLock::new()));
    }

    /// Adds a folder (its parents too).
    pub fn insert_dir(&mut self, path: &str) {
        let key = PathKey::new(path);
        if key.0.is_empty() {
            return;
        }
        self.insert_parents(&key);
        self.entries.entry(key).or_insert(Node::Dir);
    }

    fn insert_parents(&mut self, key: &PathKey) {
        for end in 1..key.0.len() {
            self.entries.entry(PathKey(key.0[..end].to_vec())).or_insert(Node::Dir);
        }
    }

    /// Every entry directly in `dir`, golden folders included, in `PathKey` order.
    fn children(&self, dir: &PathKey) -> Vec<PathKey> {
        self.entries
            .range((Bound::Excluded(dir.clone()), Bound::Unbounded))
            .map(|(key, _)| key)
            .take_while(|key| key.starts_with(dir))
            .filter(|key| key.0.len() == dir.0.len() + 1)
            .cloned()
            .collect()
    }

    /// Whether `path` is a folder named golden or lies under one.
    fn hidden(&self, path: &PathKey) -> bool {
        (1..=path.0.len()).any(|end| path.0[end - 1] == GOLDEN && self.is_dir(&PathKey(path.0[..end].to_vec())))
    }

    fn is_a_directory(&self, path: &PathKey) -> ReadError {
        ReadError::Io {
            path: path.clone(),
            message: format!("[Errno 21] Is a directory: {}", py_repr_str(&self.display(path))),
        }
    }
}

impl Tree for MemTree {
    fn kind(&self, path: &PathKey) -> Option<Kind> {
        if path.0.is_empty() {
            return Some(Kind::Dir);
        }
        self.entries.get(path).map(|node| match node {
            Node::Dir => Kind::Dir,
            Node::File(_) | Node::Lazy(..) => Kind::File,
        })
    }

    fn list(&self, dir: &PathKey) -> Vec<PathKey> {
        if !self.is_dir(dir) {
            return Vec::new();
        }
        self.children(dir).into_iter().filter(|key| !self.hidden(key)).collect()
    }

    fn glob(&self, base: &PathKey, pattern: &str) -> Vec<PathKey> {
        let parts: Vec<&str> = pattern.split('/').filter(|part| !part.is_empty() && *part != ".").collect();
        if parts.is_empty() {
            return Vec::new();
        }
        let mut current = vec![base.clone()];
        for (at, part) in parts.iter().enumerate() {
            let last = at + 1 == parts.len();
            let mut next = Vec::new();
            for dir in &current {
                if !self.is_dir(dir) {
                    continue;
                }
                if has_magic(part) {
                    for child in self.children(dir) {
                        if fnmatch(child.name(), part) && (last || self.is_dir(&child)) {
                            next.push(child);
                        }
                    }
                } else {
                    let child = dir.join(part);
                    let wanted = if last { self.exists(&child) } else { self.is_dir(&child) };
                    if wanted {
                        next.push(child);
                    }
                }
            }
            current = next;
        }
        let mut found: Vec<PathKey> = current.into_iter().filter(|path| !self.hidden(path)).collect();
        found.sort();
        found.dedup();
        found
    }

    fn read(&self, path: &PathKey) -> Result<Arc<[u8]>, ReadError> {
        match self.entries.get(path) {
            Some(Node::File(bytes)) => Ok(bytes.clone()),
            Some(Node::Lazy(load, cell)) => {
                if let Some(bytes) = cell.get() {
                    return Ok(bytes.clone());
                }
                let loaded = (**load)().map_err(|message| ReadError::Io { path: path.clone(), message })?;
                let bytes: Arc<[u8]> = Arc::from(loaded);
                Ok(cell.get_or_init(|| bytes).clone())
            }
            Some(Node::Dir) => Err(self.is_a_directory(path)),
            None if path.0.is_empty() => Err(self.is_a_directory(path)),
            None => Err(ReadError::Missing(Missing { path: path.clone(), shown: self.display(path) })),
        }
    }

    fn display(&self, path: &PathKey) -> String {
        if path.0.is_empty() {
            return self.display_root.clone();
        }
        match self.display_root.as_str() {
            "." => path.to_string(),
            "/" => format!("/{}", path),
            root => format!("{}/{}", root, path),
        }
    }

    fn resolved_name(&self, path: &PathKey) -> String {
        if path.0.is_empty() {
            self.resolved_root.clone()
        } else {
            path.name().to_string()
        }
    }
}

/// The root and the components of a path as Python 3.9's PurePosixPath parses it: exactly two leading slashes are a
/// root of their own, one or more than two are "/"; empty and "." components are dropped.
fn pathlib_parts(text: &str) -> (&'static str, Vec<&str>) {
    let root = if text.starts_with("//") && !text.starts_with("///") {
        "//"
    } else if text.starts_with('/') {
        "/"
    } else {
        ""
    };
    (root, text.split('/').filter(|part| !part.is_empty() && *part != ".").collect())
}

/// str() of a parsed path: the root and the components joined by "/", "." for nothing.
fn pathlib_join(root: &str, parts: &[&str]) -> String {
    if root.is_empty() && parts.is_empty() {
        return ".".to_string();
    }
    format!("{root}{}", parts.join("/"))
}

/// `str(PurePosixPath(text))`, Python 3.9: empty and "." components dropped, exactly two leading slashes kept (one,
/// or three or more, make one), "." for nothing.
pub fn pathlib_str(text: &str) -> String {
    let (root, parts) = pathlib_parts(text);
    pathlib_join(root, &parts)
}

/// `str(PurePosixPath(text).parent / name)`, Python 3.9: `name` beside the file `text` names (the parent is lexical:
/// the last component is replaced, "a/.." gives "a/<name>").
pub fn pathlib_str_beside(text: &str, name: &str) -> String {
    let (root, mut parts) = pathlib_parts(text);
    parts.pop();
    parts.push(name);
    pathlib_join(root, &parts)
}

/// str(Path(base) / name) for a base already as pathlib prints it.
pub fn pathlib_under(base: &str, name: &str) -> String {
    if base == "." {
        name.to_string()
    } else if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

/// Whether a glob component holds a wildcard (pathlib then lists the folder instead of testing one name).
fn has_magic(part: &str) -> bool {
    part.chars().any(|c| matches!(c, '*' | '?' | '['))
}

/// fnmatch.fnmatchcase(name, pattern): `*` any run, `?` one character, `[seq]` and `[!seq]` a set (a `]` first in
/// the set is literal, `a-z` a range, an unclosed `[` a literal), every other character literal, case-sensitive.
/// A range whose ends are reversed matches nothing (Python's re would refuse the pattern; none is used).
fn fnmatch(name: &str, pattern: &str) -> bool {
    let name: Vec<char> = name.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    match_from(&name, &pattern)
}

fn match_from(name: &[char], pattern: &[char]) -> bool {
    match pattern.first() {
        None => name.is_empty(),
        Some('*') => (0..=name.len()).any(|skip| match_from(&name[skip..], &pattern[1..])),
        Some('?') => !name.is_empty() && match_from(&name[1..], &pattern[1..]),
        Some('[') => match parse_set(&pattern[1..]) {
            Some((negated, items, used)) => {
                !name.is_empty()
                    && items.iter().any(|&(low, high)| low <= name[0] && name[0] <= high) != negated
                    && match_from(&name[1..], &pattern[1 + used..])
            }
            None => name.first() == Some(&'[') && match_from(&name[1..], &pattern[1..]),
        },
        Some(c) => name.first() == Some(c) && match_from(&name[1..], &pattern[1..]),
    }
}

/// The set that starts after a '[': (negated, its ranges, the pattern characters used through the closing ']'), or
/// None when no ']' closes it. The scan is fnmatch.translate's: a '!' then a ']' right after the '[' belong to the set.
fn parse_set(rest: &[char]) -> Option<(bool, Vec<(char, char)>, usize)> {
    let mut end = 0;
    if rest.get(end) == Some(&'!') {
        end += 1;
    }
    if rest.get(end) == Some(&']') {
        end += 1;
    }
    while end < rest.len() && rest[end] != ']' {
        end += 1;
    }
    if end >= rest.len() {
        return None;
    }
    let (negated, body) = match rest[..end].split_first() {
        Some(('!', body)) => (true, body),
        _ => (false, &rest[..end]),
    };
    let mut items = Vec::new();
    let mut at = 0;
    while at < body.len() {
        if at + 2 < body.len() && body[at + 1] == '-' {
            items.push((body[at], body[at + 2]));
            at += 3;
        } else {
            items.push((body[at], body[at]));
            at += 1;
        }
    }
    Some((negated, items, end + 1))
}

/// `tree.read` then `decode_text`; Err is the ReadError's or the decode error's text.
/// Py: Path.read_text(encoding, errors).
pub fn read_text(tree: &dyn Tree, path: &PathKey, encoding: Encoding, errors: Errors) -> Result<String, String> {
    let bytes = tree.read(path).map_err(|error| error.to_string())?;
    decode_text(&bytes, encoding, errors)
}

/// A JSON file read as "utf-8-sig", strict, into `T`; Err is the read, decode or serde_json text.
/// Py: json.loads(path.read_text(encoding="utf-8-sig")).
pub fn read_json<T: serde::de::DeserializeOwned>(tree: &dyn Tree, path: &PathKey) -> Result<T, String> {
    let text = read_text(tree, path, Encoding::Utf8Sig, Errors::Strict)?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn keys(paths: &[&str]) -> Vec<PathKey> {
        paths.iter().map(|path| PathKey::new(path)).collect()
    }

    /// A recording with the two pairs of names that sort differently as paths and as strings, an empty folder, and
    /// golden folders at two depths.
    fn sample() -> MemTree {
        let mut tree = MemTree::new("corpus/owner", "owner");
        tree.insert_file("disc-a/scan.txt", "MSG:1005,0,1,\"made-up line\"\n");
        tree.insert_file("disc-a/all-tracks/rip-summary.json", "{}");
        tree.insert_file("disc-a/all-tracks-2/rip-summary.json", "{}");
        tree.insert_file("disc-a/all-tracks/audio-levels-20ms-1-2/main-mix.txt", "x");
        tree.insert_file("disc-a/all-tracks/audio-levels-20ms-1-2-3/main-mix.txt", "x");
        tree.insert_file("disc-a/exp02/title-10.txt", "");
        tree.insert_file("disc-a/exp02/title-2.txt", "");
        tree.insert_file("disc-a/exp02/titles.txt", "");
        tree.insert_dir("disc-a/empty");
        tree.insert_file("disc-a/golden/rip-summary.json", "{}");
        tree.insert_file("disc-a/golden/scan.json", "{}");
        tree.insert_file("disc-a/all-tracks/golden/audio-x.json", "{}");
        tree
    }

    #[test]
    fn path_keys_order_component_by_component_as_python_sorts_paths() {
        let pairs = [
            ("all-tracks/x", "all-tracks-2/x"),
            ("audio-levels-20ms-1-2/main-mix.txt", "audio-levels-20ms-1-2-3/main-mix.txt"),
        ];
        for (first, second) in pairs {
            assert!(PathKey::new(first) < PathKey::new(second), "{} sorts before {}", first, second);
            assert!(first > second, "as one string {} would sort after {}", first, second);
        }
        assert!(PathKey::new("B") < PathKey::new("a"), "by code point, capitals first");
        assert!(PathKey::new("a") < PathKey::new("a/b"));
    }

    #[test]
    fn pathlib_str_keeps_exactly_two_leading_slashes_as_python_3_9_does() {
        // Checked against Python 3.9's str(PurePosixPath(s)).
        let cases = [
            ("//x/m.json", "//x/m.json"),
            ("///x", "/x"),
            ("////x", "/x"),
            ("/x", "/x"),
            ("a//b", "a/b"),
            ("./logs//extract.txt", "logs/extract.txt"),
            ("a/", "a"),
            ("", "."),
            (".", "."),
            ("/", "/"),
            ("a/..", "a/.."),
        ];
        for (given, shown) in cases {
            assert_eq!(pathlib_str(given), shown, "{given:?}");
        }
        assert_eq!(pathlib_str_beside("//x/m.json", "rip.txt"), "//x/rip.txt");
        assert_eq!(pathlib_str_beside("a/", "rip.txt"), "rip.txt");
        assert_eq!(pathlib_str_beside("", "rip.txt"), "rip.txt");
    }

    #[test]
    fn pathlib_under_puts_a_name_under_a_base_as_pathlib_printed_it() {
        assert_eq!(pathlib_under(".", "audio-fitted.json"), "audio-fitted.json");
        assert_eq!(pathlib_under("/", "audio-fitted.json"), "/audio-fitted.json");
        assert_eq!(pathlib_under("corpus/owner", "audio-fitted.json"), "corpus/owner/audio-fitted.json");
    }

    #[test]
    fn path_key_new_join_parent_and_name_behave_as_pathlib() {
        assert_eq!(PathKey::new("./a//b/"), PathKey::new("a/b"));
        assert_eq!(PathKey::new("a/b").components(), &["a".to_string(), "b".to_string()]);
        assert_eq!(PathKey::new("a").join("b/c").to_string(), "a/b/c");
        assert_eq!(PathKey::new("a/b").parent(), PathKey::new("a"));
        assert_eq!(PathKey::root().parent(), PathKey::root());
        assert_eq!(PathKey::new("a/b.txt").name(), "b.txt");
        assert_eq!(PathKey::root().name(), "");
        assert_eq!(PathKey::root().to_string(), "");
        assert_eq!(PathKey::new("."), PathKey::root());
    }

    #[test]
    fn list_gives_entries_in_python_order_with_empty_folders() {
        let tree = sample();
        assert_eq!(
            tree.list(&PathKey::new("disc-a")),
            keys(&["disc-a/all-tracks", "disc-a/all-tracks-2", "disc-a/empty", "disc-a/exp02", "disc-a/scan.txt"])
        );
        assert!(tree.is_dir(&PathKey::new("disc-a/empty")));
        assert!(tree.list(&PathKey::new("disc-a/empty")).is_empty());
        assert!(tree.is_file(&PathKey::new("disc-a/scan.txt")));
        assert!(tree.list(&PathKey::new("disc-a/scan.txt")).is_empty());
        assert!(!tree.exists(&PathKey::new("disc-a/nothing")));
        assert!(tree.is_dir(&PathKey::root()));
        assert_eq!(tree.list(&PathKey::root()), keys(&["disc-a"]));
    }

    #[test]
    fn golden_folders_are_skipped_by_list_and_glob_but_still_readable() {
        let tree = sample();
        assert!(!tree.list(&PathKey::new("disc-a")).contains(&PathKey::new("disc-a/golden")));
        assert_eq!(
            tree.glob(&PathKey::new("disc-a"), "*/rip-summary.json"),
            keys(&["disc-a/all-tracks/rip-summary.json", "disc-a/all-tracks-2/rip-summary.json"])
        );
        assert_eq!(
            tree.glob(&PathKey::root(), "*/*/*.json"),
            keys(&["disc-a/all-tracks/rip-summary.json", "disc-a/all-tracks-2/rip-summary.json"])
        );
        assert_eq!(
            tree.glob(&PathKey::new("disc-a/all-tracks"), "*"),
            keys(&[
                "disc-a/all-tracks/audio-levels-20ms-1-2",
                "disc-a/all-tracks/audio-levels-20ms-1-2-3",
                "disc-a/all-tracks/rip-summary.json",
            ])
        );
        assert!(tree.list(&PathKey::new("disc-a/golden")).is_empty());
        assert!(tree.glob(&PathKey::new("disc-a/golden"), "*").is_empty());
        assert!(tree.is_dir(&PathKey::new("disc-a/golden")));
        assert_eq!(&*tree.read(&PathKey::new("disc-a/golden/scan.json")).unwrap(), b"{}");
    }

    #[test]
    fn glob_matches_each_component_as_fnmatch() {
        let tree = sample();
        let exp = PathKey::new("disc-a/exp02");
        assert_eq!(tree.glob(&exp, "title-[0-9]*.txt"), keys(&["disc-a/exp02/title-10.txt", "disc-a/exp02/title-2.txt"]));
        assert_eq!(tree.glob(&exp, "title?.txt"), keys(&["disc-a/exp02/titles.txt"]));
        assert_eq!(tree.glob(&exp, "[!t]*"), Vec::<PathKey>::new());
        assert_eq!(
            tree.glob(&PathKey::new("disc-a"), "all-tracks/audio-levels-20ms-*"),
            keys(&["disc-a/all-tracks/audio-levels-20ms-1-2", "disc-a/all-tracks/audio-levels-20ms-1-2-3"])
        );
        assert_eq!(tree.glob(&PathKey::new("disc-a"), "scan.txt/*"), Vec::<PathKey>::new());
        assert_eq!(tree.glob(&PathKey::new("disc-a"), "scan.txt"), keys(&["disc-a/scan.txt"]));
        assert_eq!(
            tree.glob(&PathKey::new("disc-a"), "*/audio-levels-20ms-1-2/main-mix.txt"),
            keys(&["disc-a/all-tracks/audio-levels-20ms-1-2/main-mix.txt"])
        );
    }

    #[test]
    fn fnmatch_handles_sets_ranges_negation_and_literal_brackets() {
        assert!(fnmatch("abc", "a*"));
        assert!(fnmatch("abc", "*c"));
        assert!(fnmatch("", "*"));
        assert!(!fnmatch("abc", "a?"));
        assert!(fnmatch("a]", "a[]]"));
        assert!(fnmatch("ab", "a[!]]"));
        assert!(!fnmatch("a]", "a[!]]"));
        assert!(fnmatch("a-", "a[x-]"));
        assert!(fnmatch("a[b", "a[b"));
        assert!(fnmatch("a[!", "a[!"));
        assert!(!fnmatch("a[", "a[!"));
        assert!(!fnmatch("ABC", "abc"));
        assert!(fnmatch(".hidden", "*"));
        assert!(fnmatch("b", "[a-c]"));
        assert!(!fnmatch("d", "[a-c]"));
        assert!(fnmatch("-", "[a-c-e]"));
        assert!(fnmatch("e", "[a-c-e]"));
        assert!(!fnmatch("d", "[a-c-e]"));
        assert!(fnmatch("a\\b", "a\\b"));
        assert!(!fnmatch("ab", "a\\b"));
    }

    #[test]
    fn display_joins_the_display_root_as_pathlib_does() {
        let tree = MemTree::new("corpus/owner", "owner");
        assert_eq!(tree.display(&PathKey::new("disc-a/scan.txt")), "corpus/owner/disc-a/scan.txt");
        assert_eq!(tree.display(&PathKey::root()), "corpus/owner");
        let here = MemTree::new(".", "recordings");
        assert_eq!(here.display(&PathKey::new("disc-a/scan.txt")), "disc-a/scan.txt");
        assert_eq!(here.display(&PathKey::root()), ".");
        assert_eq!(MemTree::new("./recs//", "recs").display(&PathKey::new("x")), "recs/x");
        assert_eq!(MemTree::new("", "recs").display(&PathKey::new("x")), "x");
        assert_eq!(MemTree::new("/", "").display(&PathKey::new("x")), "/x");
        assert_eq!(MemTree::new("/data/recs", "recs").display(&PathKey::new("x")), "/data/recs/x");
        assert_eq!(here.resolved_name(&PathKey::root()), "recordings");
        assert_eq!(here.resolved_name(&PathKey::new("disc-a")), "disc-a");
    }

    #[test]
    fn reading_a_missing_file_or_a_folder_gives_the_python_errors() {
        let tree = sample();
        let missing = tree.read(&PathKey::new("disc-a/nothing.json")).unwrap_err();
        assert_eq!(
            missing,
            ReadError::Missing(Missing {
                path: PathKey::new("disc-a/nothing.json"),
                shown: "corpus/owner/disc-a/nothing.json".to_string(),
            })
        );
        assert_eq!(missing.to_string(), "[Errno 2] No such file or directory: 'corpus/owner/disc-a/nothing.json'");
        assert_eq!(
            tree.read(&PathKey::new("disc-a/exp02")).unwrap_err().to_string(),
            "[Errno 21] Is a directory: 'corpus/owner/disc-a/exp02'"
        );
    }

    #[test]
    fn lazy_files_are_loaded_once_and_report_their_error() {
        let calls = Arc::new(AtomicUsize::new(0));
        let counter = calls.clone();
        let mut tree = MemTree::new(".", "rec");
        tree.insert_lazy(
            "levels/main-mix.txt",
            Arc::new(move || -> Result<Vec<u8>, String> {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(b"-23.5\r\n".to_vec())
            }),
        );
        tree.insert_lazy(
            "levels/cand-2.txt",
            Arc::new(|| -> Result<Vec<u8>, String> { Err("[Errno 13] Permission denied: 'levels/cand-2.txt'".to_string()) }),
        );
        assert!(tree.is_dir(&PathKey::new("levels")));
        assert!(tree.is_file(&PathKey::new("levels/main-mix.txt")));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        for _ in 0..2 {
            assert_eq!(&*tree.read(&PathKey::new("levels/main-mix.txt")).unwrap(), b"-23.5\r\n");
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            tree.read(&PathKey::new("levels/cand-2.txt")).unwrap_err().to_string(),
            "[Errno 13] Permission denied: 'levels/cand-2.txt'"
        );
    }

    #[test]
    fn read_text_and_read_json_decode_as_python_does() {
        let mut tree = MemTree::new(".", "rec");
        tree.insert_file("a.json", b"\xef\xbb\xbf{\"ok\": true}".to_vec());
        tree.insert_file("bad.txt", b"x\xff".to_vec());
        assert_eq!(
            read_text(&tree, &PathKey::new("a.json"), Encoding::Utf8Sig, Errors::Strict),
            Ok("{\"ok\": true}".to_string())
        );
        let value: serde_json::Value = read_json(&tree, &PathKey::new("a.json")).unwrap();
        assert_eq!(value, serde_json::json!({"ok": true}));
        assert_eq!(
            read_text(&tree, &PathKey::new("bad.txt"), Encoding::Utf8, Errors::Strict),
            Err("'utf-8' codec can't decode byte 0xff in position 1: invalid start byte".to_string())
        );
        assert_eq!(
            read_text(&tree, &PathKey::new("bad.txt"), Encoding::Utf8, Errors::Replace),
            Ok("x\u{fffd}".to_string())
        );
        assert_eq!(
            read_json::<serde_json::Value>(&tree, &PathKey::new("none.json")),
            Err("[Errno 2] No such file or directory: 'none.json'".to_string())
        );
    }
}
