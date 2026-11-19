# KT-215 — vá bộ lọc thư mục build `target/` (đường lọt #12)

**Vai**: L47 — tác giả bản vá (tiếp KT-212/213).
**Nhánh**: `night3/L47-kt212`, gốc `aebe5505` (KT-213).
**Worktree**: `/private/tmp/night3-L47-kt212`.
**Nguồn**: đường lọt #12 do chính L47 dựng ở KT-212 (`kt212-attack-path-mod.md` §5).

## 0. Kết quả

| Hạng mục | Trước | Sau |
|---|---|---|
| Repro #12 (thường) | **XANH** | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |
| Repro #12 (`--strict`) | **XANH** | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |
| Đối chứng dương: tệp trong `v2/kernel/target/` (build dir) | bị bỏ qua | **vẫn bị bỏ qua** (XANH) |
| Cây thật (thường / `--strict`) | XANH | **XANH** |
| `--selftest` | 165/165 | **176/176** (thêm 11 ca KT-215) |
| Báo oan mới trên cây thật | — | **0** |

## 1. Gốc lỗi

**Năm** chỗ quét dùng `"target" in p.parts`:

- `iter_rs()` — nguồn `files` của **mọi** luật;
- `_protected_files()` — `scan_lexical`;
- `scan_structure()`;
- `scan_macro_bodies()`;
- `_shadowed_safe_macros()`.

Bộ lọc nhằm bỏ thư mục build của cargo (`v2/kernel/target/`), nhưng khớp **bất kỳ** thành phần
`target` nào — kể cả `src/target/`. Tệp trong `src/target/` **không** vào `files`, **không** vào
`_protected_files`, **không** vào `scan_structure`; `resolve_include_targets` cũng không seed nó
(seed từ `files`) nên **không** vào `extra`. `_mods_of` (đồ thị test-only) **có** đọc nó nhưng chỉ
để tính test-only — không kéo vào tập bị quét.

## 2. Repro (đo được)

```rust
// v2/kernel/vifast-core/src/lib.rs
mod target;
```

```rust
// v2/kernel/vifast-core/src/target/mod.rs
mod evil;
```

```rust
// v2/kernel/vifast-core/src/target/evil.rs
use crate::{KError, Machine, ObjId};
pub fn evil(m: &mut Machine, t: ObjId) -> Result<(), KError> {
    m.wake(t)
}
```

| Lệnh | Trước KT-215 | Sau KT-215 |
|---|---|---|
| `cargo check -p vifast-core` | RC=0 | RC=0 (mã vẫn biên dịch; checker mới là thứ đỏ) |
| checker thường | **XANH** | **ĐỎ** |
| checker `--strict` | **XANH** | **ĐỎ** |

Log: `kt215-logs/repro12-cargo.log`, `repro12-after-normal.log`, `repro12-after-strict.log`.

## 3. Bản vá

### 3.1 Một hàm dùng chung

```python
def _is_build_dir(root: Path, p: Path) -> bool:
    try:
        parts = p.relative_to(root).parts
    except ValueError:
        return False          # ngoài root ⇒ KHÔNG bỏ qua (fail-closed)
    if "target" not in parts:
        return False
    ti = parts.index("target")
    if "src" not in parts:
        return True           # <crate>/target/… — build dir
    return ti < parts.index("src")
```

Thay **cả năm** biểu thức rời bằng `_is_build_dir(root, p)` — đúng bài học "một định nghĩa,
không chép". Đường dẫn ngoài `root` ⇒ **không** bỏ qua (quét thừa còn hơn bỏ sót).

### 3.2 Đóng F2 (hệ quả bắt buộc của tiêu chí "ĐỎ cả hai chế độ")

Sau khi `src/target/evil.rs` vào `files`, `--strict` ĐỎ ngay (luật `wake` quét `files`), nhưng
chế độ **thường** vẫn XANH: luật `wake` ở chế độ thường chỉ quét `extra` (tệp bị kéo vào bằng
`#[path]`/`include!`) — đúng **F2** đã ghi ở hợp đồng §4. `src/target/evil.rs` là module **bình
thường**, không bao giờ vào `extra`.

Nay chế độ thường quét **mọi** tệp sản phẩm trong phạm vi (`files` + `extra`), cùng tập đích với
`--strict`; `--strict` chỉ còn thêm allowlist `transition_tcb`. Đây là **đóng F2**, không phải nới
ngưỡng: chỉ **siết** (thêm tệp bị quét), và cây thật vẫn xanh vì `--strict` vốn đã quét đúng tập
đó. Có `scanned` set để không báo trùng tệp nằm ở cả `files` lẫn `extra`.

### 3.3 Cùng lớp: miễn luật kênh theo **tên** `mailbox.rs`

Rà thêm: `p.name == "mailbox.rs"` (luật kênh phát/thu) khớp **mọi** tệp cùng tên ở bất kỳ thư mục
nào ⇒ một tệp `src/sub/mailbox.rs` chứa `post_mail(…, MailboxMsg::Wake(t))` được miễn luật kênh
phát (kênh wake thứ hai). Nay so **đúng relpath** của hai tệp định nghĩa trong phạm vi quét
(`MAILBOX_DEFS = (KERNEL_MAILBOX, SIM_MAILBOX)`). Cây thật chỉ có đúng hai tệp đó trong phạm vi
(`v2/lib/vifast-sync/src/mailbox.rs` nằm ngoài `V2_SCAN`) ⇒ không báo oan.

## 4. Rà các bộ lọc cùng lớp (task 4)

| Bộ lọc | Dạng | Khớp nhầm? | Quyết định |
|---|---|---|---|
| `"target" in p.parts` (năm chỗ) | thành phần **đường dẫn** | **CÓ** — `src/target/` | **Đã vá** (`_is_build_dir`) |
| `p.name == "mailbox.rs"` (hai chỗ, luật kênh) | **tên tệp** | **CÓ** — mọi thư mục | **Đã vá** (`MAILBOX_DEFS`) |
| `p.name.startswith("virtio_")` (luật arch) | **tiền tố tên** | **CÓ** — `virtio_x.rs` | **Chưa vá** — nợ có tên: phân biệt "thiết bị virtio thật" cần kiểm ngữ nghĩa (kiểu `Virtio*`), không phải đổi tên; luật arch chỉ áp `vifast-plat-*`, rủi ro thấp |
| `"tests"`, `"fixtures"`, `"build"`, `".git"`, `"node_modules"` | — | **KHÔNG có** trong checker | Không cần sửa |
| `is_test_file` / `test_only_modules` | **đồ thị module** (KT-171/172) | Không — không dựa tên/đường dẫn | Không cần sửa |
| `in_test_only_wake_scope` | regex **phạm vi** (`<crate>/src/**`, `build.rs`) | Không — là định nghĩa phạm vi, không phải bỏ qua | Không cần sửa |
| `_inside_scan_tree` / `_inside_lib_tree` | tiền tố **phạm vi** | Không | Không cần sửa |
| `rglob("*.rs")` | đuôi tệp | Không — `#[path]` tới tệp không `.rs` vẫn được phân giải qua `is_file()` | Không cần sửa |

## 5. Ca selftest (thêm 11, tổng 176)

| Ca | Kỳ vọng |
|---|---|
| repro #12 `src/target/evil.rs` chứa `wake` (thường) | ĐỎ |
| repro #12 (`--strict`) | ĐỎ |
| đối chứng dương: tệp trong `v2/kernel/target/` bị bỏ qua (thường) | XANH |
| đối chứng dương: như trên (`--strict`) | XANH |
| `_is_build_dir(v2/kernel/target/x.rs)` | True |
| `_is_build_dir(v2/kernel/vifast-core/target/x.rs)` | True |
| `_is_build_dir(v2/kernel/vifast-core/src/target/x.rs)` | False |
| `_is_build_dir(v2/kernel/vifast-core/src/foo/target/x.rs)` | False |
| `_is_build_dir(v2/lib/vifast-abi/src/target/x.rs)` | False |
| `_is_build_dir(v2/kernel/vifast-core/src/foo.rs)` | False |
| `src/sub/mailbox.rs` (cùng tên, khác thư mục) phát `MailboxMsg::Wake` | ĐỎ `E_CHANNEL_PRODUCER` |

## 6. Đo sau vá

| Lệnh | Kết quả | Log |
|---|---|---|
| `python3 v2/tools/check-wake-paths.py --root .` | XANH | `kt215-logs/after-normal.log` |
| `… --strict` | XANH | `kt215-logs/after-strict.log` |
| `… --selftest` | 176/176 ok | `kt215-logs/after-selftest.log` |
| `python3 v2/tools/verify-numbers.py --root .` | ok=5 fail=0 | `kt215-logs/verify-numbers.log` |
| `./v2/bin/v2 check` | xem `kt215-logs/v2-check.log` | — |

## 7. Không hồi quy

- 165 ca cũ **giữ nguyên**, không ca nào bị bỏ/xoá; tổng 176.
- Cây thật `errors=0` cả hai chế độ.
- Đóng F2 **không** làm đỏ ca nào (đã chạy lại toàn bộ selftest).

## 8. Tệp chạm

- `v2/tools/check-wake-paths.py` — `_is_build_dir` (mới) + năm call site; `MAILBOX_DEFS` + hai
  call site; đóng F2 ở khối `if not strict:`; 11 ca selftest.
- `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` — §4: đóng F2, thêm dòng `target/`, thêm dòng
  `mailbox.rs`, ghi nợ `virtio_`.
- `v2/agent/JOURNAL/kt215-logs/` — log.
