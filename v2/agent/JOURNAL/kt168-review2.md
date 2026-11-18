# KT-168 — review đối kháng vòng hai: KT-165 (luật `wake`) + KT-166 (KT-158/160)

**Ngày**: 2026-11-19 (vòng hai) · **Reviewer L11** (độc lập, không phải lead/tác giả) · nhánh
`night2/L11-review2` · worktree `/private/tmp/night2-L11-review2` · HEAD `25306b3f` (từ `main`).
**Nguồn đọc trước**: [`kt165-wakefix.md`](kt165-wakefix.md), [`kt166-gapfix.md`](kt166-gapfix.md).
**Ngân sách**: hạn cứng 07:00 JST. Không merge, không push, không sửa `main`, không `gate --fast`,
không tạo worktree mới. Mọi đột biến đã `git checkout --`; cây sạch trước khi commit.

## 0. Kết luận trước

| Task | Phán quyết | Căn cứ (một dòng) |
|---|---|---|
| **KT-165** | **KHÔNG ĐẠT** | 6/6 lỗ hổng được giao (H1..H6) **đã đóng** — kiểm lại độc lập; nhưng luật vẫn **lọt 5 lớp**, trong đó **B8/B9 là lỗ hổng thật**: đặt `wake` trong module sản phẩm tên `ktest*` hoặc dưới `src/tests/` ⇒ ảnh sản phẩm biên dịch `wake` mà `--strict` vẫn `errors=0`. |
| **KT-166** | **ĐẠT** | KT-158: đột biến bỏ `Cwd` ⇒ ca 16 **ĐỎ 4 phép kiểm** (tái lập độc lập, ảnh `sha256=b71e57f7…`); `.`/`..` phủ bởi `ls .`/`cd /README/..`/`cat ../README` (M2/M3 ĐỎ). KT-160: **4/4** đột biến bit khác tác giả (`DESC_AF`, `DESC_PXN`, `DESC_UXN`, `DESC_SH`) đều ĐỎ test literal; 2 đột biến tác giả + xoá table-at-leaf ĐỎ. L0: `v2/bin/v2:427` có `-p vifast-plat-aarch64`, lệnh chạy **24 test**. |

**Phát hiện nặng nhất**: `is_test_file` (`v2/tools/check-wake-paths.py:357`) miễn theo **tên/đường
dẫn** (`name.startswith("ktest")`, `"/tests/" in relp`), không theo `#[cfg(test)]`. Một module
**sản phẩm** tên `ktest_evil.rs` (hoặc `src/tests/evil.rs` với `mod tests;` không `#[cfg(test)]`)
được biên dịch vào ảnh sản phẩm, gọi `m.wake(t)`, mà `--strict` vẫn `errors=0` — đúng thứ luật
D2-144 sinh ra để chặn. Đây là lỗ hổng **thật**, không phải lý thuyết: đã `cargo check` PASS.

## 1. A — KT-165: thử lách luật `E_STRICT_TEST_ONLY_WAKE` (cách thứ 7)

### 1.1 Bảng thử (mọi log ở `v2/agent/JOURNAL/kt168-logs/`)

| # | Cách thử | Kết quả | `file:dòng` + log |
|---|---|---|---|
| **B1** | `Machine::wake` 4 biến thể: liền, `Machine:: wake`, `Machine :: wake`, `Machine::` ⏎ `wake` (trong `core_sched.rs`) | **BỊ BẮT** | 4× `E_STRICT_TEST_ONLY_WAKE` @ `core_sched.rs:246,247,248,250` — `b1-use-machine.log` |
| **B2** | trait/generic: `trait WakeApi { fn wake(&mut self, t: ObjId); }` + `fn f<M: WakeApi>(m: &mut M) { m.wake(t) }` + UFCS `WakeApi::wake(m, t)` | **BỊ BẮT** | 2× `E_STRICT_TEST_ONLY_WAKE` @ `core_sched.rs:1006` (gọi `wake(`), `:1007` (`::wake`) — `b2-trait.log` |
| **B3** | `macro_rules!` **trong phạm vi**: thân `$m.wake($t)`; và thân `$m.$name($t)` với `wake` làm ident | **BỊ BẮT** | 2× `E_STRICT_TEST_ONLY_WAKE` @ `core_sched.rs:1005` (gọi), `:1007` (giá trị) — `b3-macro-in-scope.log` |
| **B4** | `#[macro_export] macro_rules! wake_call` đặt trong **`v2/lib/vifast-abi/src/lib.rs`** (ngoài phạm vi quét), gọi `vifast_abi::wake_call!(m, t)` trong `core_sched.rs` | **LỌT** | macro @ `vifast-abi/src/lib.rs:128` (`$m.wake($t)`), gọi @ `core_sched.rs:1005`; `--strict` `errors=0` — `b4-macro-outscope.log`; `cargo check -p vifast-core` **PASS** — `b4-cargo-check.log` |
| **B5** | `m.wake(t)` trong `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs` | **KHÔNG QUÉT** (không phải lỗ hổng thật) | `--strict` `errors=0` — `h4-init-not-scanned.log`; nhưng `init/` biên dịch bằng `rustc` trực tiếp, `--extern` **không** có `vifast_core` (`build.rs:116-194`) ⇒ lời gọi **không biên dịch được**. Chỉ thành lỗ hổng khi `#[path]`/`include!` từ `src/` (B6/B7). |
| **B6** | `include!("../../../lib/vifast-abi/src/evil_include.rs")` trong `core_sched.rs`; tệp ngoài phạm vi chứa `m.wake(t)` | **LỌT** | include @ `core_sched.rs:1005`, `wake` @ `vifast-abi/src/evil_include.rs:3`; `--strict` `errors=0` — `b6-include.log`; `cargo check -p vifast-core` **PASS** — `b6-cargo-check.log` |
| **B7** | `#[path = "../../../lib/vifast-abi/src/evil_path.rs"] mod _b7;` trong `core_sched.rs`; tệp ngoài phạm vi chứa `m.wake(t)` | **LỌT** | `#[path]` @ `core_sched.rs:1005`, `wake` @ `vifast-abi/src/evil_path.rs:6`; `--strict` `errors=0` — `b7-path-module.log`; `cargo check -p vifast-core` **PASS** — `b7-cargo-check.log` |
| **B8** | Module **sản phẩm** tên `ktest_evil.rs` + `mod ktest_evil;` trong `main.rs` (ảnh sản phẩm) | **LỌT** | `mod` @ `main.rs:39`, `wake` @ `ktest_evil.rs:7`; `--strict` `errors=0` — `b8-ktest-name.log`; `cargo check -p vifast-kbin-riscv64 --target riscv64gc-unknown-none-elf` **PASS** — `b8-cargo-check.log` |
| **B9** | Module **sản phẩm** trong `src/tests/` + `mod tests;` **không** `#[cfg(test)]` | **LỌT** | `mod tests;` @ `lib.rs:51`, `wake` @ `src/tests/evil.rs:4`; `--strict` `errors=0` — `b9-src-tests-dir.log`; `cargo check -p vifast-core` **PASS** — `b9-cargo-check.log` |
| **B10** | `macro_rules!` nhận ident + biến cục bộ `let mut wake` (miễn theo **cả hàm**) | **LỌT** | macro @ `core_sched.rs:1005`, `let mut wake` @ `:1007`, gọi `_b10!(m, wake, t)` @ `:1009`; `--strict` `errors=0` — `b10-local-macro.log`; `cargo check -p vifast-core` **PASS** (1 warning) — `b10-cargo-check.log` |
| **H2** | `m.wake(t)` trong `vifast-plat-riscv64/src/lib.rs` (kiểm phạm vi plat) | **BỊ BẮT** | `E_STRICT_TEST_ONLY_WAKE` @ `vifast-plat-riscv64/src/lib.rs:903` — `h2-plat-scope.log` |
| **H4+H6** | `m.wake /* H4 */ (t)` trong `mod` đứng sau `#[cfg(test)] use …;` | **BỊ BẮT** | `E_STRICT_TEST_ONLY_WAKE` @ `core_sched.rs:1009` — `h4-h6-recheck.log` |

### 1.2 Vì sao B4/B6/B7/B8/B9/B10 lọt

- **B4** — `TEST_ONLY_WAKE_SCOPE` chỉ quét `v2/kernel/…` + `v2/sim/…`; thân macro nằm trong
  `v2/lib/vifast-abi/src/lib.rs` (ngoài phạm vi). Lời gọi `vifast_abi::wake_call!(m, t)` **không
  chứa token `wake`** (`wake_call` có `_` sau `wake` ⇒ regex token không khớp). Đây đúng là hở
  "macro_rules sinh lời gọi" mà tác giả ghi còn lý thuyết — nhưng bản **ngoài phạm vi** thì
  **cụ thể**, biên dịch được, không cần `quote!`/`concat_idents!`.
- **B6/B7** — `iter_rs` chỉ đọc tệp **trong** các thư mục phạm vi; `include!`/`#[path]` trỏ ra
  ngoài (kể cả `init/`) không được phân giải và quét. `#[path]` **đã có trong cây**
  (`v2/kernel/vifast-kbin-riscv64/src/ktest_vfs_rig.rs:22` trỏ `../init/pager_common.rs`), nên
  đây không phải mã kỳ dị.
- **B8/B9** — `is_test_file` (`check-wake-paths.py:357-366`) miễn theo **tên/đường dẫn**:
  `name.startswith("ktest")`, `"/tests/" in relp`, `name.endswith("_tests.rs")`. Không có bước
  kiểm "tệp này chỉ được `mod` từ ảnh test". Một module sản phẩm đặt tên `ktest_*.rs` hoặc trong
  `src/tests/` (với `mod tests;` **không** `#[cfg(test)]`) được biên dịch vào ảnh sản phẩm nhưng
  bị miễn — **lỗ hổng thật**, không phải lý thuyết.
- **B10** — `_fn_has_local_wake` miễn **cả hàm** khi thấy `let (mut) wake` ở **bất kỳ** scope con;
  macro nhận `$name:ident` biến `wake` (không có `(` ngay sau) thành lời gọi method. Đây là hệ quả
  của miễn-local-binding quá rộng, không phải lỗi mới của KT-165.

### 1.3 Sáu lỗ hổng được giao — kiểm lại (đều ĐÓNG)

| Lỗ hổng | Cách kiểm | Kết quả |
|---|---|---|
| H1 `wake(` trong `vifast-kbin-riscv64/src/` | đột biến `m.wake(t)` trong `proc_rt.rs:214` | **ĐỎ** @ `proc_rt.rs:214` — `h1-kbin-scope.log` |
| H2 `wake(` trong `vifast-plat-*/src/` | `m.wake(t)` trong `vifast-plat-riscv64/src/lib.rs` | **ĐỎ** @ `:903` — `h2-plat-scope.log` |
| H3 alias `Machine::wake` | 4 biến thể B1 | **ĐỎ** — `b1-use-machine.log` |
| H4 block comment `wake /*x*/ (t)` | H4+H6 | **ĐỎ** @ `:1009` — `h4-h6-recheck.log` |
| H5 gọi cắt dòng `wake` ⏎ `(t)` | B1d | **ĐỎ** @ `:250` — `b1-use-machine.log` |
| H6 `#[cfg(test)]` trên `use` nuốt `mod` | H4+H6 | **ĐỎ** @ `:1009` — `h4-h6-recheck.log` |

Cây sạch: `--strict` `errors=0` (`clean-strict.log`); `--selftest` **26/26** (`selftest.log`);
`./v2/bin/v2 check` **PASS** (`v2-check-clean.log`).

## 2. B — KT-166: thử phá bằng chứng mới

### 2.1 KT-158 — đường tương đối trong `/docs`

Ảnh dựng lại từ cây hiện tại (`25306b3f`) **trùng byte** ảnh tác giả:
`sha256=85a80b429729e802b07329725a3045910fbb7f61d953941992021058475b2bda size=213632`
(`/tmp/l11-target-fs`, `--features product-fs`). Ảnh sạch: **15/15 PASS**, `shell-edge.sh: PASS`
(`shell-edge-clean.log`).

| Đột biến | Ảnh | Kết quả ca 16 |
|---|---|---|
| **M1** `resolve_path` bỏ `Cwd` (`shell_user.rs:472-475` → `out[0]=b'/'; len=1;`) | `sha256=b71e57f76862f89508981b6d263e9bf859813d3a2ebaf363d89d9f7b4d2c7e33` | **ĐỎ** — `ls-cwd` (thấy 3 dòng `path=/`), `ls-docs` (1 thay vì 2), `cat-docs` (1 thay vì 2), `stat-docs` (0 thay vì 1); `shell-edge.sh: FAIL` — `shell-edge-mut1.log` |
| **M2** `.` thành tên thật (`shell_user.rs:486` bỏ `comp == b"."`) | `sha256=41bd17d9d8139787a53f46e7a8dd3965aab169e9e810f9719396d0b05adcaab1` | **ĐỎ** — `ls-cwd` (1 thay vì 2), `ls-docs` (1 thay vì 2) — `shell-edge-mut2.log` ⇒ `./`/`.` phủ bởi `ls .` |
| **M3** `..` thành tên thật (`shell_user.rs:489-499` bỏ nhánh lùi) | `sha256=272b56d76475fdb5b43302beef5beda083baad5d0943f062d1105d00567fc3a8` | **ĐỎ** — `pwd` (6 thay vì 8 dòng `/`), `cd-thu-tu` (chuỗi `cd` lệch), `cat-tuong-doi` (1 thay vì 2) — `shell-edge-mut3.log` ⇒ `..` phủ bởi `cd /README/..`, `cd /khongco/..`, `cd /docs/..`, `cat ../README` |

**Ba dạng đường trong đề** (`cat ./note.txt`, `cat ../docs/note.txt`, `cd /docs/..` rồi
`cat note.txt`): **không** có nguyên văn trong ca 16, nhưng **cùng đường mã** đã bị M2/M3 bắt:
`.` phủ bởi `ls .` (2 lần), `..` phủ bởi `cd /README/..` + `cd /khongco/..` + `cd /docs/..` +
`cat ../README`; `cd /docs/..` rồi `cat note.txt` phủ bởi đếm `pwd` (đúng 2 dòng `/docs`) và
`cd-thu-tu`. Ghi chú nhỏ: dạng `..` **giữa đường** (`../docs/note.txt`) không được kích hoạt
**trực tiếp**; implementation xử lý `..` đồng nhất mọi vị trí nên không có đột biến tự nhiên nào
phân biệt — **không phải lỗ hổng**, chỉ là ghi chú phủ.

### 2.2 KT-160 — bit literal ARMv8-A + bảng-mức-lá

Baseline: `cargo test -p vifast-plat-aarch64` ⇒ **24 passed** (`a64-baseline.log`).

| Đột biến (bit **khác** tác giả) | Kết quả |
|---|---|
| `DESC_AF: 1<<10` → `1<<12` (`format.rs:91`) | `descriptor_bits_literal_armv8a` **FAILED** (`host.rs:304`) + 4 test khác — `a64-mut-af.log` |
| `DESC_PXN: 1<<53` → `1<<52` (`format.rs:99`) | `descriptor_bits_literal_armv8a` **FAILED** (`host.rs:304`), 23 passed — `a64-mut-pxn.log` |
| `DESC_UXN: 1<<54` → `1<<55` (`format.rs:101`) | `descriptor_bits_literal_armv8a` **FAILED** (`host.rs:304`), 23 passed — `a64-mut-uxn.log` |
| `DESC_SH_SHIFT: 8` → `9` (`format.rs:89`) | `descriptor_bits_literal_armv8a` **FAILED** (`host.rs:304`) + `descriptor_bits_aarch64` — `a64-mut-sh.log` |
| (tác giả) `DESC_ATTR_SHIFT: 2` → `3` | literal **FAILED** (`host.rs:304`), 23 passed — `a64-mut-attr.log` |
| (tác giả) `DESC_NG: 1<<11` → `1<<60` | literal **FAILED** (`host.rs:344`), 23 passed — `a64-mut-ng.log` |
| (tác giả) xoá `if level + 1 == F::LEVELS { return Err(CorruptTable) }` (`mmu.rs:340`) | `translate_table_at_leaf_rejected` **FAILED** (`host.rs:778`) — `a64-mut-table-at-leaf.log` |

⇒ **Không bit nào vẫn xanh.** Test literal dùng bảng `mod armv8a` viết tay + **giá trị tuyệt đối**
(`0x0060_0000_8000_1707` / `0x0020_0000_8000_1FC7`), nên mọi thay đổi bit trong hai descriptor
đều làm đỏ — đúng mục tiêu T1. Ca `TableAtLeaf4` chạm đúng nhánh phòng thủ generic (AArch64
`decode` không bao giờ trả `Table` ở mức lá — đúng đặc tả), đột biến xoá nhánh ⇒ ĐỎ — đúng T2.

### 2.3 L0 — `-p vifast-plat-aarch64`

- `v2/bin/v2:427` (trong `do_gate_fast`): `run_step "L0 kernel workspace (KT-148)" "$CARGO" test
  --manifest-path "$V2_ROOT/kernel/Cargo.toml" -p vifast-plat-common -p vifast-plat-aarch64`.
- `vifast-plat-aarch64` là member workspace kernel (`v2/kernel/Cargo.toml:10`).
- Chạy **đúng lệnh** (không chạy `gate --fast` theo luật task): **RC=0**, `vifast-plat-aarch64`
  `tests/host.rs` **24 passed**, `vifast-plat-common` host 14 + 11 file test khác đều `ok` —
  `l0-kernel-workspace.log`.
- `./v2/bin/v2 check` **PASS** (`v2-check-clean.log`) — nhưng `check` **không** chạy bước L0 kernel
  workspace (bước đó chỉ nằm trong `gate --fast`); đây là lý do tác giả thêm `-p` vào `gate`.

## 3. C — Phán quyết

### KT-165 — **KHÔNG ĐẠT**

- **Đã đóng**: 6/6 lỗ hổng được giao (H1..H6) — kiểm lại độc lập, đều ĐỎ đúng mã
  `E_STRICT_TEST_ONLY_WAKE`; cây sạch `errors=0`; selftest 26/26; `v2 check` PASS.
- **Chưa đóng**: luật vẫn lọt **5 lớp** (B4, B6, B7, B8/B9, B10). **B8/B9 là lỗ hổng thật**:
  - B8: `v2/kernel/vifast-kbin-riscv64/src/ktest_evil.rs:7` + `main.rs:39` — ảnh **sản phẩm**
    biên dịch `m.wake(t)`, `--strict` `errors=0`.
  - B9: `v2/kernel/vifast-core/src/tests/evil.rs:4` + `lib.rs:51` (`mod tests;` **không**
    `#[cfg(test)]`) — module được biên dịch vào sản phẩm, `--strict` `errors=0`.
- **Vì sao KHÔNG ĐẠT**: mục tiêu của KT-165 là "**mọi** tham chiếu `wake` trong **mã sản phẩm**
  ⇒ ĐỎ" (docstring checker). B8/B9 chứng minh mục tiêu đó **chưa đạt** bằng một thủ thuật
  **đơn giản** (đổi tên tệp / đặt dưới `src/tests/`), không cần mã kỳ dị. Cần vòng vá 7:
  - `is_test_file` phải dựa trên **`#[cfg(test)]`/`mod tests`** (hoặc xác minh tệp chỉ được
    `mod` từ ảnh test), không dựa tên/đường dẫn;
  - quét **thân macro** ở mọi crate mà sản phẩm phụ thuộc (hoặc cấm `#[macro_export]` sinh
    `wake`), và phân giải `include!`/`#[path]` để quét tệp đích;
  - thu hẹp miễn-local-binding theo **scope** thay vì cả hàm.

### KT-166 — **ĐẠT**

- KT-158: đột biến bỏ `Cwd` ⇒ ca 16 **ĐỎ 4 phép kiểm** (tái lập độc lập, ảnh trùng byte
  `85a80b42…`); `.`/`..` phủ bởi `ls .`/`cd /README/..`/`cat ../README` (M2/M3 ĐỎ). Ba dạng
  đường trong đề được phủ **gián tiếp** qua cùng đường mã — ghi chú, không phải lỗ hổng.
- KT-160: 4/4 đột biến bit khác tác giả đều ĐỎ test literal; 2 đột biến tác giả + xoá
  table-at-leaf ĐỎ; baseline 24 passed.
- L0: `-p vifast-plat-aarch64` nằm trong `v2/bin/v2:427`, lệnh chạy 24 test, RC=0.

## 4. D — Còn thiếu / chưa review

- **Không** chạy `gate --fast` (luật task) — chỉ chạy đúng lệnh cargo của bước L0 kernel
  workspace + `v2 check`.
- **Không** kiểm lại toàn bộ 15 ca `shell-edge` dưới từng đột biến M2/M3 (chỉ ca 16 + tổng kết);
  các ca khác không liên quan `resolve_path`.
- **Không** thử `macro_rules!` với `quote!`/`paste!` (không có dependency trong cây) — B4 đã đủ
  chứng minh lớp lỗ hổng macro ngoài phạm vi.
- **Không** thử `#[cfg(test)]` range với brace imbalance (mã Rust hợp lệ không có) — đã đọc mã,
  không thấy đường lách.
- **Không** sửa mã sản phẩm trên `main`; mọi đột biến trong worktree đã `git checkout --`; cây
  sạch trước commit (chỉ thêm journal + log).

## 5. E — Luật đã giữ

- Không merge, không push, không sửa `main`, không `gate --fast`, không tạo worktree mới.
- Không `pkill -f qemu-system`; mỗi lượt QEMU do `shell-edge.sh`/`qemu_guard.sh` quản lý, tối đa
  **1** QEMU; `pgrep -c qemu-system` = 0 trước/sau mỗi lượt; đã dọn sạch.
- Không nới ngưỡng/checker, không `#[ignore]`/`#[should_panic]`, không retry để lấy xanh.
- `export PATH="$HOME/.cargo/bin:$PATH"`; bash 3.2 macOS: không `timeout`, không `local a=1 b="$a"`.
- Mọi log trong `v2/agent/JOURNAL/kt168-logs/` (36 tệp).
