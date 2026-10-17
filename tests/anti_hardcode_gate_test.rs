use std::fs;
use std::path::Path;

/// Quality Gate: Quét toàn bộ mã nguồn production trong `src/` và `ts/` để đảm bảo:
/// 1. KHÔNG chứa bất kỳ khối hardcode từ điển ánh xạ ngữ nghĩa nào (như `if lower.contains("claim")` hay `replace("biên lại", "biên lai")`).
/// 2. KHÔNG chứa các ví dụ dịch thuật cố định trong prompt.
/// 3. Khả năng hiểu ngữ nghĩa phải dựa 100% vào mô hình Embedding Vector, BM25, và RRF Fusion.
#[test]
fn test_no_hardcoded_semantic_thesaurus_in_core_src_and_ts() {
    let forbidden_patterns = [
        "if lower.contains(\"claim\")",
        "if lower.contains(\"invoice\")",
        "biên lại\", \"biên lai\"",
        "bien lai\", \"biên lai\"",
        "\"hóa đơn\", include \"invoice\"",
    ];

    let mut violations = Vec::new();

    fn scan_dir(dir: &Path, forbidden: &[&str], violations: &mut Vec<String>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let path_str = path.to_string_lossy().to_string();
                if path_str.contains("tests") || path_str.contains("node_modules") || path_str.contains("target") {
                    continue;
                }
                if path.is_dir() {
                    scan_dir(&path, forbidden, violations);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs")
                    || path.extension().and_then(|s| s.to_str()) == Some("ts")
                {
                    if let Ok(content) = fs::read_to_string(&path) {
                        for pattern in forbidden {
                            if content.contains(pattern) {
                                violations.push(format!(
                                    "File '{}' contains forbidden hardcoded pattern '{}'",
                                    path.display(),
                                    pattern
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    scan_dir(Path::new("src"), &forbidden_patterns, &mut violations);
    scan_dir(Path::new("ts"), &forbidden_patterns, &mut violations);

    assert!(
        violations.is_empty(),
        "Anti-Hardcode Quality Gate Failed! Found violations:\n{}",
        violations.join("\n")
    );
}
