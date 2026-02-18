use std::path::Path;
use tempfile::TempDir;

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let rel = path.strip_prefix(from).unwrap();
        let target = to.join(rel);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::copy(path, target).unwrap();
    }
}

pub fn copy_claude_fixture_tree(temp: &TempDir) {
    let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("claude")
        .join("projects");
    let target_root = temp.path().join("projects");
    copy_tree(&fixture_root, &target_root);
}
