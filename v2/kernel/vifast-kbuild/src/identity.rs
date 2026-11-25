// SPDX-License-Identifier: MIT OR Apache-2.0
//! **KT-404** — danh tính bản dựng nhúng vào ảnh (`KT-86`/D2-125.3, `KT-89`/D2-127, `KT-143`).
//!
//! # Vì sao
//!
//! Bốn lần artifact lệch mã nguồn (D2-108A/KT-71/76/79/77b) đều được phát hiện **sau** khi đo.
//! Từ `KT-86` ảnh tự mang `git rev` + cờ dirty của **lúc build**; kernel in một dòng
//! `# build: algo=… inputs=… dirty=…` (từ `const`, D2-102 — không literal) và harness
//! `v2/test/boot/build-rev.sh` **từ chối đo** khi ảnh lệch `HEAD` hoặc dirty=1.
//!
//! Định nghĩa `dirty` (nguồn sự thật duy nhất: `v2/test/boot/build-rev.sh`, cùng phạm vi):
//! chỉ tính `v2/kernel` + `v2/lib` + `v2/user`, **có** tính tệp chưa track, **không** tính
//! `v2/docs` / `v2/agent/JOURNAL` (nếu tính, mỗi lần ghi log sẽ làm mọi phép đo bị từ chối).
//!
//! Fail-closed: không đọc được rev ⇒ `unknown`; không chứng minh được là sạch ⇒ `1`. Cả hai đều
//! làm cổng **từ chối**, không bao giờ dẫn tới "nhận nhầm là sạch".
//!
//! # `KT-404` — gộp ba bản thành một
//!
//! Trước `KT-404` khối này lặp **3 lần** (~230 LOC/bản) trong ba `build.rs` và **đã trôi**:
//! bản `riscv64` tính `dirty` trên `v2/kernel v2/lib` **thiếu `v2/user`**, trong khi `v2/user`
//! **nằm trong** `VBREV_INPUT_ROOTS` của `build-rev.sh` (KT-346) ⇒ sửa `v2/user` làm dấu vân tay
//! `inputs` đổi nhưng ảnh riscv64 vẫn khai `dirty=0`. Từ đây **một** định nghĩa cho cả 3 arch:
//! `v2/kernel v2/lib v2/user` (khớp `VBREV_INPUT_ROOTS`), tức **siết** chứ không nới.

use std::path::{Path, PathBuf};
use std::process::Command;

/// `rev` khi không đọc được `git` (không phải repo, không có git): harness thấy giá trị này là
/// **từ chối đo** — cố ý, không có nhánh "đo được dù không biết nguồn".
const UNKNOWN_REV: &str = "unknown";

/// Số ký tự hex của `rev` trong dòng bằng chứng (khớp `VBREV_REV_LEN` của harness).
const REV_LEN: usize = 12;

/// Dấu vân tay build-inputs khi không tính được (khớp `VBREV_INPUTS_LEN` = 16 của harness).
const UNKNOWN_INPUTS: &str = "unknown";

/// **KT-143** — tên **định nghĩa** dấu vân tay build-inputs (`VBREV_ALGO` của harness), in vào
/// dòng `# build:` để một log tự khai nó được tính bằng **công thức nào**. Vì sao cần: đổi định
/// nghĩa (thêm/bớt thư mục khỏi phạm vi) làm dấu vân tay đổi mà **không** đổi một byte mã nguồn
/// nào — nhìn log cũ và log mới thấy hai số khác nhau mà không có cách nào biết chúng có so được
/// với nhau hay không. Có `algo=` thì câu trả lời nằm ngay trong log.
const UNKNOWN_ALGO: &str = "unknown";

/// Phạm vi tính `dirty` — **một** nguồn cho cả 3 arch, khớp `VBREV_INPUT_ROOTS` của
/// `v2/test/boot/build-rev.sh` (`v2/kernel v2/lib v2/user`).
const DIRTY_SCOPE: [&str; 3] = ["v2/kernel", "v2/lib", "v2/user"];

/// `true` ⇔ `s` là một tên định nghĩa hợp lệ (chữ/số/`-`/`_`/`.`, không rỗng, ≤ 32 ký tự).
fn is_algo(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
}

/// `true` ⇔ `s` là 16 ký tự hex thường (đúng độ dài `VBREV_INPUTS_LEN`).
fn is_inputs16(s: &str) -> bool {
    s.len() == 16
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Đọc `VBREV_ALGO` bằng **chính** `v2/test/boot/build-rev.sh` (một nguồn sự thật — không chép
/// lại tên định nghĩa vào build script). Trả `None` khi không chạy được ⇒ ảnh mang
/// `algo=unknown` ⇒ cổng **từ chối** (fail-closed).
fn algo_from_script(root: &Path) -> Option<String> {
    let script = root.join("v2/test/boot/build-rev.sh");
    if !script.is_file() {
        return None;
    }
    let out = Command::new("bash")
        .arg("-c")
        .arg(". \"$1\" && printf '%s' \"$VBREV_ALGO\"")
        .arg("bash")
        .arg(&script)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    if is_algo(&s) {
        Some(s)
    } else {
        None
    }
}

/// Tính dấu vân tay build-inputs bằng **chính** `v2/test/boot/build-rev.sh` (một nguồn sự thật).
/// Trả `None` khi không chạy được ⇒ ảnh mang `inputs=unknown` ⇒ cổng **từ chối** (fail-closed).
fn inputs_from_script(root: &Path) -> Option<String> {
    let script = root.join("v2/test/boot/build-rev.sh");
    if !script.is_file() {
        return None;
    }
    let out = Command::new("bash")
        .arg("-c")
        .arg(". \"$1\" && vbrev_inputs_calc \"$2\" && printf '%s' \"$VBREV_CALC_SHORT\"")
        .arg("bash")
        .arg(&script)
        .arg(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    if is_inputs16(&s) {
        Some(s)
    } else {
        None
    }
}

/// Đặt `VIFAST_BUILD_REV`/`VIFAST_BUILD_DIRTY`/`VIFAST_BUILD_INPUTS`/`VIFAST_BUILD_ALGO` cho
/// crate (⇒ `env!` trong `src/main.rs`) và khai báo mọi thứ phải làm build script **chạy lại** để
/// giá trị không bị cũ.
///
/// `manifest_dir` = `CARGO_MANIFEST_DIR` của crate ảnh (`v2/kernel/vifast-kbin-<arch>`);
/// `arch_crate` = tên thư mục crate ảnh (`vifast-kbin-<arch>`) — **khác biệt duy nhất** giữa ba
/// arch trong khối này.
pub fn build_identity(manifest_dir: &str, arch_crate: &str) {
    // Gốc cây nguồn = `<manifest>/../../..` (v2/kernel/vifast-kbin-<arch> → gốc repo).
    let root = std::fs::canonicalize(Path::new(manifest_dir).join("../../.."))
        .unwrap_or_else(|_| PathBuf::from(manifest_dir).join("../../.."));
    // Env do `v2/tools/repro-build.sh` đặt được **ưu tiên**: giá trị của nó do
    // `v2/test/boot/build-rev.sh` tính (nguồn sự thật duy nhất của định nghĩa dirty) ⇒ dòng
    // `REPRO` của script và dòng `# build:` của ảnh **không thể** lệch định nghĩa.
    let rev = std::env::var("VIFAST_BUILD_REV")
        .ok()
        .map(|v| normalize_rev(&v))
        .filter(|v| is_rev12(v))
        .unwrap_or_else(|| git_head12(&root).unwrap_or_else(|| UNKNOWN_REV.to_string()));
    let dirty = std::env::var("VIFAST_BUILD_DIRTY")
        .ok()
        .filter(|v| v == "0" || v == "1")
        .unwrap_or_else(|| git_dirty(&root));
    // D2-127: danh tính **quyết định** là dấu vân tay **nội dung build-inputs** (không phải rev).
    // Nguồn sự thật duy nhất: `vbrev_inputs_calc` của `v2/test/boot/build-rev.sh` — build script
    // **gọi chính script đó** thay vì chép lại định nghĩa (A0: danh sách do script sinh, không viết tay).
    let inputs = std::env::var("VIFAST_BUILD_INPUTS")
        .ok()
        .filter(|v| is_inputs16(v))
        .unwrap_or_else(|| inputs_from_script(&root).unwrap_or_else(|| UNKNOWN_INPUTS.to_string()));
    // KT-143: tên **định nghĩa** dấu vân tay, cùng nguồn sự thật với `inputs`.
    let algo = std::env::var("VIFAST_BUILD_ALGO")
        .ok()
        .filter(|v| is_algo(v))
        .unwrap_or_else(|| algo_from_script(&root).unwrap_or_else(|| UNKNOWN_ALGO.to_string()));
    println!("cargo:rustc-env=VIFAST_BUILD_REV={rev}");
    println!("cargo:rustc-env=VIFAST_BUILD_DIRTY={dirty}");
    println!("cargo:rustc-env=VIFAST_BUILD_INPUTS={inputs}");
    println!("cargo:rustc-env=VIFAST_BUILD_ALGO={algo}");
    println!(
        "cargo:warning=KT-89 build identity algo={algo} inputs={inputs} dirty={dirty} rev={rev}"
    );
    // Bản build **sau** phải tính lại danh tính: env đổi, HEAD/index đổi, hoặc mã nguồn trong
    // phạm vi dirty đổi. Thiếu các dòng này cargo dùng lại build script cũ ⇒ ảnh mang rev cũ
    // (đúng lớp lỗi KT-76). Mọi bất định chỉ dẫn tới **từ chối đo**, không dẫn tới nhận nhầm.
    println!("cargo:rerun-if-env-changed=VIFAST_BUILD_REV");
    println!("cargo:rerun-if-env-changed=VIFAST_BUILD_DIRTY");
    // D2-127: dấu vân tay build-inputs cũng là **đầu vào** — thiếu dòng này thì cargo dùng lại
    // build script cũ ⇒ ảnh mang dấu vân tay cũ (đúng lớp lỗi KT-76).
    println!("cargo:rerun-if-env-changed=VIFAST_BUILD_INPUTS");
    // KT-143: tên định nghĩa cũng là **đầu vào** — thiếu dòng này thì cargo dùng lại build script
    // cũ ⇒ ảnh mang `algo=` cũ (đúng lớp lỗi KT-76).
    println!("cargo:rerun-if-env-changed=VIFAST_BUILD_ALGO");
    for path in identity_watch_paths(&root, arch_crate) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// `git -C <root> <args>`: stdout đã trim khi exit 0, `None` khi không chạy được/lỗi.
fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("--no-optional-locks")
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// 12 ký tự hex đầu của `HEAD` (thường hoá thường). `None` khi không có git/repo.
fn git_head12(root: &Path) -> Option<String> {
    let head = git_stdout(root, &["rev-parse", "--verify", "HEAD"])?;
    let head = head.to_ascii_lowercase();
    if head.len() < REV_LEN || !head.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(head[..REV_LEN].to_string())
}

/// `"1"` khi `v2/kernel`/`v2/lib`/`v2/user` có thay đổi chưa commit (kể cả tệp chưa track);
/// **mọi bất định ⇒ `"1"`** (`git` lỗi, không có git, không phải repo) — fail-closed như harness.
fn git_dirty(root: &Path) -> String {
    let mut args: Vec<&str> = vec![
        "status",
        "--porcelain",
        "--untracked-files=normal",
        "--ignore-submodules=all",
        "--",
    ];
    args.extend(DIRTY_SCOPE);
    match git_stdout(root, &args) {
        Some(out) if out.is_empty() => "0".to_string(),
        _ => "1".to_string(),
    }
}

/// Danh sách đường dẫn phải làm build script chạy lại (giữ danh tính không bị cũ).
///
/// KHÔNG watch cả `v2/kernel`: `v2/kernel/target/` nằm trong đó, mỗi lần build lại đổi ⇒ build
/// script (và ~20 ELF U-mode) chạy lại mọi lần. Watch từng crate + `v2/lib` + `v2/user` là đủ
/// phạm vi dirty.
fn identity_watch_paths(root: &Path, arch_crate: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(git_dir) = git_stdout(root, &["rev-parse", "--absolute-git-dir"]) {
        out.push(Path::new(&git_dir).join("HEAD"));
    }
    if let Some(index) = git_stdout(root, &["rev-parse", "--git-path", "index"]) {
        let index = Path::new(&index);
        out.push(if index.is_absolute() {
            index.to_path_buf()
        } else {
            root.join(index)
        });
    }
    for rel in [
        arch_crate,
        "v2/kernel/vifast-core",
        "v2/kernel/vifast-plat-common",
        "v2/kernel/vifast-kbuild",
        "v2/user",
        "v2/kernel/Cargo.toml",
        "v2/kernel/Cargo.lock",
        "v2/lib",
    ] {
        out.push(root.join(rel));
    }
    out
}

/// 12 hex thường của `rev` (env có thể là SHA đầy đủ hoặc có khoảng trắng/xuống dòng).
fn normalize_rev(value: &str) -> String {
    let hex: String = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_ascii_hexdigit())
        .take(REV_LEN)
        .collect();
    hex
}

/// `true` nếu đúng 12 ký tự hex (định dạng dòng bằng chứng của harness).
fn is_rev12(value: &str) -> bool {
    value.len() == REV_LEN && value.chars().all(|c| c.is_ascii_hexdigit())
}
