use std::fs;
use std::path::{Path, PathBuf};

use coding_tools_mcp_desktop_lib::tools::policy::{validate_tool_arguments, PolicySettings};
use coding_tools_mcp_desktop_lib::tools::{call_tool, ToolContext};
use serde_json::{json, Value};

pub struct FixtureWorkspace {
    pub root: PathBuf,
    pub outside_secret: PathBuf,
    _temp: tempfile::TempDir,
}

pub fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../old/tests/compliance/fixtures")
}

pub fn tiny_js_fixture() -> FixtureWorkspace {
    prepare_fixture("tiny-js-project", false)
}

pub fn malicious_fixture() -> FixtureWorkspace {
    prepare_fixture("malicious-project", true)
}

fn prepare_fixture(name: &str, symlink_escape: bool) -> FixtureWorkspace {
    let temp = tempfile::tempdir().expect("tempdir");
    let parent = temp.path();
    let source = fixtures_root().join(name);
    assert!(source.is_dir(), "missing fixture: {}", source.display());
    let root = parent.join(name);
    copy_dir_all(&source, &root).expect("copy fixture");
    let outside_secret = parent.join("outside-secret.txt");
    fs::write(
        &outside_secret,
        fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../old/tests/compliance/outside-secret.txt"),
        )
        .expect("outside-secret.txt"),
    )
    .expect("write outside secret");
    materialize_runtime_files(&root, &outside_secret, name);
    if symlink_escape {
        let link = root.join("outside-link.txt");
        let _ = fs::remove_file(&link);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside_secret, &link).expect("symlink");
        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_file(&outside_secret, &link).is_err() {
                eprintln!("skip symlink setup on windows");
            }
        }
    }
    FixtureWorkspace {
        root,
        outside_secret,
        _temp: temp,
    }
}

fn materialize_runtime_files(root: &Path, outside_secret: &Path, name: &str) {
    let reference = root.join(".reference");
    let _ = fs::create_dir_all(&reference);
    let _ = fs::write(reference.join("cache.txt"), "reference cache must be excluded\n");
    let _ = fs::create_dir_all(root.join("node_modules/leftpad"));
    let _ = fs::write(root.join("node_modules/leftpad/index.js"), "module.exports = 1;\n");
    let _ = fs::create_dir_all(root.join("dist"));
    let _ = fs::write(root.join("dist/bundle.js"), "bundle output must be excluded\n");
    let _ = fs::write(root.join("ignored.log"), "ignored by fixture gitignore\n");
    if name == "tiny-js-project" {
        let _ = fs::create_dir_all(root.join("assets"));
        let _ = fs::write(root.join("assets/raw.bin"), b"\x00\xff\x00binary\x00");
        let _ = fs::write(root.join("src/large.txt"), "0123456789abcdef\n".repeat(256));
        let _ = fs::create_dir_all(root.join("search"));
        for index in 0..12 {
            let _ = fs::write(
                root.join(format!("search/bulk_{index:02}.txt")),
                format!("common-token bulk line {index}\n"),
            );
        }
    }
    let _ = outside_secret;
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if file_type.is_symlink() {
            #[cfg(unix)]
            {
                let link = fs::read_link(entry.path())?;
                std::os::unix::fs::symlink(link, target)?;
            }
            #[cfg(windows)]
            {
                let link = fs::read_link(entry.path())?;
                if link.is_dir() {
                    std::os::windows::fs::symlink_dir(link, target)?;
                } else {
                    std::os::windows::fs::symlink_file(link, target)?;
                }
            }
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn ctx_for(root: &Path) -> ToolContext {
    ToolContext::new(root.to_path_buf()).expect("tool context")
}

pub fn invoke(ctx: &ToolContext, name: &str, args: Value) -> Value {
    call_tool(ctx, name, &args)
}

pub fn assert_ok(result: &Value) -> &Value {
    assert_eq!(result.get("ok"), Some(&json!(true)), "{result}");
    result
}

pub fn assert_err(result: &Value) -> &Value {
    assert_eq!(result.get("ok"), Some(&json!(false)), "{result}");
    let err = result.get("error").expect("error object");
    assert!(err.get("code").and_then(Value::as_str).is_some());
    assert!(err.get("message").and_then(Value::as_str).is_some());
    assert!(err.get("category").and_then(Value::as_str).is_some());
    assert!(err.get("retryable").map(Value::is_boolean).unwrap_or(false));
    assert!(err.get("details").map(Value::is_object).unwrap_or(false));
    result
}

pub fn assert_security_or_policy_err(result: &Value) {
    let err = assert_err(result);
    let cat = err["error"]["category"].as_str().unwrap_or("");
    assert!(
        matches!(cat, "security" | "policy" | "permission" | "validation"),
        "unexpected category: {cat}"
    );
}

pub fn assert_policy_rejects(tool: &str, args: Value) {
    let policy = PolicySettings::default();
    assert!(validate_tool_arguments(tool, &args, &policy).is_err());
}
