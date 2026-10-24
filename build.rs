use std::path::{Path, PathBuf};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    let version = env!("CARGO_PKG_VERSION");
    let build = build_id(root, version);
    println!("cargo:rustc-env=BEST_LOUPE_BUILD={build}");

    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    embed_manifest(root, &out_dir);
    embed_version_resource(&out_dir, version, &build);
}

/// `git describe` of the source tree. Builds run from an rsynced copy without `.git`, so
/// `scripts/win-build.sh` drops the describe string into `.build-describe`; plain git is the fallback.
fn build_id(root: &Path, version: &str) -> String {
    let file = root.join(".build-describe");
    println!("cargo:rerun-if-changed={}", file.display());
    let describe = std::fs::read_to_string(&file)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            let out = std::process::Command::new("git")
                .args(["describe", "--tags", "--always", "--dirty"])
                .current_dir(root)
                .output()
                .ok()?;
            out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
        })
        .unwrap_or_else(|| format!("v{version}-unknown"));
    // A build exactly on (or after) a release tag must match the Cargo.toml version.
    if let Some(tag) = describe.strip_prefix('v') {
        let tag_ver = tag.split('-').next().unwrap_or("");
        if tag_ver.split('.').count() == 3 && tag_ver != version {
            println!("cargo:warning=git tag v{tag_ver} does not match Cargo.toml version {version}");
        }
    }
    describe
}

fn embed_manifest(root: &Path, out_dir: &Path) {
    let src = root.join("app.manifest");
    println!("cargo:rerun-if-changed={}", src.display());
    // `uiaccess` feature: deployable build (must be signed + installed under Program Files).
    // Without it the exe runs from anywhere, but not over elevated windows / Start.
    let ui_access = std::env::var_os("CARGO_FEATURE_UIACCESS").is_some();
    let text = std::fs::read_to_string(&src).expect("read app.manifest");
    let text = text.replace("{{UI_ACCESS}}", if ui_access { "true" } else { "false" });
    let out = out_dir.join("app.manifest");
    std::fs::write(&out, text).expect("write manifest");
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    // Our manifest carries requestedExecutionLevel; the linker's default UAC fragment would conflict.
    println!("cargo:rustc-link-arg-bins=/MANIFESTUAC:NO");
    println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", out.display());
}

/// VERSIONINFO resource (Explorer → Properties → Details), compiled with the SDK's rc.exe.
fn embed_version_resource(out_dir: &Path, version: &str, build: &str) {
    let mut nums = version.split('.').map(|p| p.parse::<u16>().unwrap_or(0));
    let (major, minor, patch) = (nums.next().unwrap_or(0), nums.next().unwrap_or(0), nums.next().unwrap_or(0));
    let rc = format!(
        r#"1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "artyfarty"
      VALUE "FileDescription", "best_loupe - hold-to-zoom lens magnifier"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "best_loupe"
      VALUE "OriginalFilename", "best_loupe.exe"
      VALUE "ProductName", "best_loupe"
      VALUE "ProductVersion", "{build}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    );
    let rc_path = out_dir.join("resources.rc");
    let res_path = out_dir.join("resources.res");
    std::fs::write(&rc_path, rc).expect("write resources.rc");
    let rc_exe = find_rc_exe().expect("rc.exe not found (Windows SDK); set RC_EXE");
    let status = std::process::Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_path)
        .arg(&rc_path)
        .status()
        .expect("run rc.exe");
    assert!(status.success(), "rc.exe failed");
    println!("cargo:rustc-link-arg-bins={}", res_path.display());
}

fn find_rc_exe() -> Option<PathBuf> {
    println!("cargo:rerun-if-env-changed=RC_EXE");
    if let Some(p) = std::env::var_os("RC_EXE") {
        return Some(p.into());
    }
    let bin = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = std::fs::read_dir(bin)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join(r"x64\rc.exe").exists())
        .collect();
    versions.sort();
    versions.pop().map(|p| p.join(r"x64\rc.exe"))
}
