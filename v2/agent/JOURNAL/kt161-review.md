# KT-161 — review đối kháng KT-158 / KT-159 / KT-160 (đêm 2026-11-19, vòng hai)

**Ngày**: 2026-11-19 · **Reviewer độc lập L6** (không phải lead, không phải tác giả) · nhánh
`night2/L6-review` · worktree `/private/tmp/night2-L6-review` · HEAD `9a8b8acb` (tách từ `main`
@ `9a8b8acb`) · range review `6b2fdce7..9a8b8acb`.

**Luật đã giữ**: không merge/push/sửa `main`; không `gate --fast`; không tạo worktree mới; không
`pkill -f qemu-system`; tối đa 1 QEMU của mình (mở khi `pgrep -c qemu-system` = 0, đóng trước khi
kết thúc); không nới ngưỡng/checker; không `#[ignore]`. Mọi đột biến áp vào worktree rồi
`git checkout --` — cây sạch (`git status` rỗng) trước khi commit.

**Cách đọc**: mỗi kết luận dưới đây chỉ dựa trên **đo lại** trong worktree này, không dựa trên
journal của tác giả. Log đột biến ở `v2/agent/JOURNAL/kt161-logs/`.

---

## 0. Phán quyết

| Task | Phán quyết | Lý do một dòng |
|---|---|---|
| **KT-158** | **ĐẠT CÓ ĐIỀU KIỆN** | Seed `/docs` + `/docs/note.txt` có thật; ca 16 đủ **14** phép kiểm; đột biến `pwd` in hằng `/` **ĐỎ** đúng; **nhưng** đột biến "`resolve_path` bỏ qua `Cwd`" **vẫn XANH** — đường tương đối từ thư mục con vẫn chưa được kiểm. |
| **KT-159** | **ĐẠT CÓ ĐIỀU KIỆN** | Luật `E_STRICT_TEST_ONLY_WAKE` chạy trong `v2 check`; `--selftest` **15/15** và thật sự gọi checker; 6 ca ghim xanh; 2 đột biến ngữ nghĩa **ĐỎ** đúng; **nhưng** luật bị lách **6** cách — nặng nhất: **không quét `vifast-kbin-riscv64/src` (ảnh sản phẩm) và `vifast-plat-riscv64/src`**. |
| **KT-160** | **ĐẠT CÓ ĐIỀU KIỆN** | Crate thật, 22 test xanh, build `aarch64-unknown-none` **thật** (`Compiling` + probe `compile_error!`), 3 đột biến encoder **ĐỎ**; **nhưng** test descriptor **tự so hằng số của chính crate** (đổi vị trí bit `AttrIndx`/`nG` vẫn 22/22 xanh), `translate` bỏ kiểm bảng-mức-lá **không test nào bắt**, và crate **không** nằm trong `gate --fast`. |

**Phát hiện nặng nhất (KT-159)**: luật cưỡng chế `wake()` chỉ-dùng-cho-test **không phủ crate ảnh
sản phẩm**. Đặt `m.wake(t)` trong `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs` (hoặc
`vifast-plat-riscv64/src/lib.rs`) **biên dịch được** cho `riscv64gc-unknown-none-elf` và
`check-wake-paths.py --strict` vẫn `errors=0`. Đây đúng là nơi một lời gọi sản phẩm sẽ được viết.

---

## 1. KT-158 — seed `/docs` + ca 16 (11→14 phép kiểm)

### 1.1 Tác giả nhận

- `proc_fs.rs::seed()` thêm `/docs` (`INODE_DIR`) + `/docs/note.txt` (`INODE_FILE`, 16 B
  `ghi chu cua docs`); `used=2 → 4`.
- `shell-edge.sh` ca 16: **11 → 14** phép kiểm (`cd-docs`, `ls-docs`, `cat-docs`); `pwd` đòi
  **8** dòng `pwd /` + **2** dòng `pwd /docs`; `cd-thu-tu` so **11** dòng.
- Đột biến "`pwd` in hằng `/`" ⇒ ca 16 **ĐỎ**; ảnh sạch ⇒ **XANH**.

### 1.2 Đo lại — đối chứng dương (ảnh sạch)

```
$ bash v2/tools/repro-build.sh --target-dir /tmp/l6-target   # VIFAST_REPRO_CARGO_EXTRA=--features product-shell,product-fs
REPRO … sha256=2287ac3ade8095d21a634b72b80f80654e8c9b8ae121fba9a24cfc0e9d3dc3ec … dirty=0 rev=9a8b8acbf83d
$ bash v2/test/boot/shell-edge.sh --image /tmp/l6-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
# kt137-cdpwd[pwd]: PASS … [cd-docs]: PASS [ls-docs]: PASS [cat-docs]: PASS
# kt137-cdpwd: PASS (# shell: pwd /)
# shell-edge.sh: PASS
```

14/14 phép kiểm ca 16 **PASS** trên ảnh sạch (`kt161-logs/kt158-clean.log`). FS sản phẩm thật sự
có thư mục con:

```
# product fs: medium=ram blocks=96 used=4 total=66 …
  d docs
  f note.txt
```

### 1.3 Đột biến M1 (của tác giả, kiểm lại): `pwd` in hằng `/`

`cmd_pwd` (`v2/kernel/vifast-kbin-riscv64/init/shell_user.rs:589`) bỏ `cwd.as_slice()`, in `/`.
Ảnh `sha256=d83c02b0… dirty=1`:

```
# kt137-cdpwd[pwd]: FAIL (mong đúng 8 dòng '# shell: pwd /', thấy 10)
# kt137-cdpwd: FAIL
# shell-edge.sh: FAIL
```

⇒ **ĐỎ đúng** (`kt161-logs/kt158-m1.log`). Vế `/docs` mới thật sự phân biệt được đột biến này —
trước KT-158 mọi `pwd` đều `/` nên đột biến không phân biệt được. **Xác nhận giá trị của KT-158.**

### 1.4 Đột biến M2 (của reviewer, khác tác giả): `resolve_path` bỏ qua `Cwd`

Sửa `resolve_path` (`shell_user.rs:464`): nhánh đường **tương đối** không ghép `cwd` nữa mà luôn
bắt đầu từ `/` (`out[0] = b'/'; len = 1;`). `cd` **vẫn** cập nhật `Cwd`; `pwd` **vẫn** in
`cwd`. Ảnh `sha256=34159e5c… dirty=1`:

```
# kt137-cdpwd[pwd]: PASS
# kt137-cdpwd[cd-thu-tu]: PASS
# kt137-cdpwd[cd-docs]: PASS
# kt137-cdpwd[ls-docs]: PASS
# kt137-cdpwd[cat-docs]: PASS
# kt137-cdpwd: PASS (# shell: pwd /)
# shell-edge.sh: PASS
```

⇒ **XANH — lỗ hổng phủ** (`kt161-logs/kt158-m2.log`). Nguyên nhân: khối `/docs` mới chỉ dùng
**đường tuyệt đối** (`cat /docs/note.txt`, `cd /docs/khongco`, `cd /docs/..`) và `ls` **không
tham số** (đi thẳng `cwd.as_slice()`, **không** qua `resolve_path`). Lệnh tương đối duy nhất chạy
trong `/docs` là `cd ..` — ở `/docs` hay ở `/` nó đều in `# shell: cd /`, nên **không phân biệt
được**. Toàn bộ đường tương đối còn lại (`cat README`, `stat README`, `write rel.txt`, …) chạy
khi `cwd == /`, nơi "bỏ qua `Cwd`" trùng hành vi đúng.

**Đề nghị (không tự sửa)**: thêm một lệnh tương đối **trong** `/docs`, ví dụ `cat note.txt` (sau
`cd /docs`) hoặc `cd docs` từ `/`, rồi thêm phép kiểm tương ứng. Khi đó M2 sẽ ĐỎ.

### 1.5 Kiểm 14 phép kiểm có thật sự chạy trên `/docs`

- `check_kt137_cdpwd` (`shell-edge.sh:1229`) có đúng **14** cặp phán quyết; 3 cặp mới
  `cd-docs`/`ls-docs`/`cat-docs` (`:1242-1244`) trỏ `ok_kt137_cd_docs` (`:1099`),
  `ok_kt137_ls_docs` (`:1111`), `ok_kt137_cat_docs` (`:1127`).
- `ok_kt137_pwd` (`:1027`) đếm **8** `pwd /` + **2** `pwd /docs` + tổng **10**.
- `ok_kt137_cd_thu_tu` so **11** dòng, có `# shell: cd /docs` và
  `# shell: cd khong co /docs/khongco`.
- `--selftest`: **42/42** (9/9 ca; ca 16 log sai **2 PASS + 12 FAIL**, log đúng **14 PASS + 0
  FAIL**) — khớp lời bàn giao.
- **Nhận xét nhỏ**: `ok_kt137_ls_docs`/`ok_kt137_cat_docs` grep **toàn bộ** log của ca, không
  giới hạn lát `/docs`. Hôm nay không có dòng nào khác sinh ra `  f note.txt`/`ghi chu cua docs`
  nên không false-positive, nhưng nếu sau này có lệnh khác in cùng chuỗi thì phép kiểm có thể
  xanh oan. Không đủ nặng để hạ phán quyết.

### 1.6 Kết luận KT-158

**ĐẠT CÓ ĐIỀU KIỆN.** Deliverable có thật, đột biến bắt buộc của tác giả **ĐỎ**, đối chứng dương
**XANH**, 14 phép kiểm chạy thật. Điều kiện: (i) đóng lỗ hổng M2 bằng một lệnh tương đối trong
`/docs`; (ii) (nhẹ) scope các phép kiểm `/docs` vào lát log của ca thay vì toàn log.

---

## 2. KT-159 — `wake()` chỉ-dùng-cho-test + luật `E_STRICT_TEST_ONLY_WAKE`

### 2.1 Tác giả nhận

- Doc-comment `wake()` (kernel + sim) ghi "chỉ dùng cho test".
- Luật `--strict`: lời gọi `wake(` trong `v2/kernel/vifast-core/src/` hoặc
  `v2/sim/vifast-sim/src/` ngoài `#[cfg(test)]`/`mod tests` ⇒ ĐỎ.
- `--selftest` **15/15**; test ghim `kt159_wake_semantics.rs` **6 ca**; 2 đột biến ngữ nghĩa ĐỎ.

### 2.2 Đo lại — phần ĐẠT

- `python3 v2/tools/check-wake-paths.py --selftest` ⇒ `# selftest: 15/15 ok`
  (`kt161-logs/kt159-selftest.log`).
- **Selftest thật sự chạy checker**: sửa **bản sao** checker cho
  `RE_TEST_ONLY_WAKE_CALL = re.compile(r"NEVER_MATCH_L6_XYZ")` ⇒
  `not ok - v2 --strict gọi wake( trong mã sản phẩm` và `# selftest: 14/15 ok`
  (`kt161-logs/kt159-selftest-broken.log`). Vậy ca 4f không phải "trang trí".
- Luật nằm trong cổng: `v2/bin/v2:309` chạy `check-wake-paths.py --strict` trong `v2 check`.
- Baseline test ghim: `6 passed; 0 failed` (`kt161-logs/kt159-baseline.log`).
- **Đột biến M3** — `wake()` thành post + `Ready` ngay (cả kernel `core_sched.rs:245` và sim
  `kobj.rs:5732`): `4 passed; 2 failed` — đỏ đúng
  `kt159_core_wake_cheo_hart_post_only` + `kt159_sim_wake_cheo_hart_post_only`
  (`kt161-logs/kt159-m3-wake-post-ready.log`).
- **Đột biến M4** — bỏ `push_mail(Wake)` trong `wake_from()` (cả hai tầng): `4 passed; 2 failed`
  — đỏ đúng `kt159_core_wake_from_cheo_hart_ready_ngay` +
  `kt159_sim_wake_from_cheo_hart_ready_ngay` (`kt161-logs/kt159-m4-wake-from-no-mail.log`).
- Đối chứng dương: lời gọi trực tiếp `m.wake(t)` trong `vifast-core/src/` ⇒
  `E_STRICT_TEST_ONLY_WAKE` @ `core_sched.rs:1006`, `errors=1` (`kt161-logs/kt159-bypass.log`).

### 2.3 Lỗ hổng lách luật (đo được, `kt161-logs/kt159-bypass.log`)

| # | Cách lách | Kết quả | Ghi chú |
|---|---|---|---|
| **H1** | Đặt `m.wake(t)` trong `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs` (ảnh sản phẩm) | **XANH** (`errors=0`) | `TEST_ONLY_WAKE_SCOPE` (`check-wake-paths.py:101`) chỉ có `vifast-core/src/` + `vifast-sim/src/`. Biên dịch thật cho `riscv64gc-unknown-none-elf`: `cargo check -p vifast-kbin-riscv64 --target …` **OK**. |
| **H2** | Đặt `m.wake(t)` trong `v2/kernel/vifast-plat-riscv64/src/lib.rs` | **XANH** (`errors=0`) | Cùng lý do scope; `cargo check -p vifast-plat-riscv64 --target …` **OK**. |
| **H3** | Alias/fn-pointer: `let f: fn(&mut Machine, ObjId) -> Result<(), KError> = Machine::wake; f(m, t)` trong `core_sched.rs` | **XANH** (`errors=0`) | Regex `RE_TEST_ONLY_WAKE_CALL` (`:108`) chỉ bắt `wake(`; `Machine::wake` không có `(`. `cargo check -p vifast-core` **OK**. |
| **H4** | `m.wake /* L6 */ (t)` (block comment giữa tên và `(`) | **XANH** (`errors=0`) | `strip_line_comment` (`:308`) chỉ cắt `//`, không cắt `/* */`; regex đòi `wake\s*\(`. Biên dịch **OK**. |
| **H5** | Gọi cắt dòng: `m.wake` ⏎ `(t)` | **XANH** (`errors=0`) | Checker chạy **từng dòng**; regex không bắt được qua newline. Biên dịch **OK**. |
| **H6** | `#[cfg(test)]` trên một `use`, rồi `mod sản_phẩm { … m.wake(t) … }` trong **4 dòng** kế | **XANH** (`errors=0`) | `test_line_ranges` (`:256`) thấy `#[cfg(test)]` rồi tìm `mod … {` trong `j-i <= 4` ⇒ nuốt **cả** module sản phẩm vào "vùng test". Biên dịch **OK**. |

**Đánh giá**: H1/H2 là lỗ hổng **nặng** — luật đặt đúng mục tiêu "không cho lời gọi sản phẩm"
nhưng lại **không quét crate sản phẩm**; H3–H5 là giới hạn của regex theo dòng (dễ lách, dù
rustfmt thường sẽ chuẩn hoá H4/H5); H6 là lỗi logic của `test_line_ranges` (một `#[cfg(test)]`
không phải `mod` vẫn mở vùng test). Không cái nào làm hỏng phần đã giao, nhưng chúng làm **sai**
câu "cưỡng chế bằng máy" nếu đọc như một bảo đảm tuyệt đối.

### 2.4 Kết luận KT-159

**ĐẠT CÓ ĐIỀU KIỆN.** Luật + selftest + test ghim + 2 đột biến ngữ nghĩa đều đạt; nhưng phạm vi
quét phải mở thêm `vifast-kbin-riscv64/src` (trừ `ktest*.rs`) và `vifast-plat-*/src`, và nên
chặn H3–H6 (ít nhất H6 — sửa `test_line_ranges` để chỉ nhận `#[cfg(test)]` **đứng ngay trước**
`mod`). Nếu không, "cưỡng chế" chỉ đúng với người không cố lách.

---

## 3. KT-160 — `vifast-plat-aarch64` (K4' bước 1)

### 3.1 Tác giả nhận

- Crate `v2/kernel/vifast-plat-aarch64` (`no_std`, `forbid(unsafe_code)`, 0 dependency), trait
  `PageTableFormat`/`Mmu` **định nghĩa cục bộ** vì `vifast-plat-common` không có trait này.
- 22 test host xanh; 3 đột biến (bỏ `AF`, `unmap` không `free_frame`, đảo `AP[2]`) đỏ.
- Build `aarch64-unknown-none` xanh.

### 3.2 Đo lại — phần ĐẠT

- `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64` ⇒
  `22 passed; 0 failed` (`kt161-logs/kt160-baseline.log`).
- **Build thật** (không no-op):
  `cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64 --target aarch64-unknown-none`
  in `Compiling vifast-plat-aarch64 v0.1.0 (…)` và sinh
  `v2/kernel/target/aarch64-unknown-none/debug/libvifast_plat_aarch64.rlib` (218 360 B)
  (`kt161-logs/kt160-build-aarch64.log`). Thêm probe `compile_error!("L6 probe…")` vào `lib.rs`
  ⇒ build **FAIL** đúng (`error: L6 probe: crate is really compiled`) — chứng minh cargo thật sự
  đọc/biên dịch crate, không phải cache rỗng.
- **Trait không tồn tại ở crate chung**: `grep -rn "PageTableFormat" v2/kernel/vifast-plat-common
  v2/lib/vifast-pte` ⇒ **rỗng**. Lý do định nghĩa cục bộ là **đúng sự thật** (đây là nợ có tên
  N1, không phải lỗi che giấu).
- **Đột biến encoder** (khác tác giả) — đều **ĐỎ**:
  - M5a `encode_leaf` dùng `(attr.mair_index() + 1) << DESC_ATTR_SHIFT` ⇒ 1 failed
    (`descriptor_bits_aarch64`, `host.rs:202`).
  - M6a `share.bits() << (DESC_SH_SHIFT + 1)` ⇒ 1 failed (`host.rs:207`).
  - M7a luôn đặt `DESC_NG` (kể cả kernel) ⇒ 2 failed (`host.rs:200`, `:317`).
  - M10 `MAIR_NORMAL_WB_RW_ALLOC 0xFF → 0x00` ⇒ 1 failed (`host.rs:275`).
  (`kt161-logs/kt160-mutations.log`)

### 3.3 Phát hiện T1 — test descriptor **tự so hằng số của chính crate** (tautology một phần)

`descriptor_bits_aarch64` (`host.rs:181`) và `permissions_roundtrip` (`:282`) kiểm bit bằng
**chính** các hằng `DESC_*` mà `encode_leaf` dùng (`format.rs:78-110`, `:380`). Vì vậy đổi **vị
trí** một bit (không đè trường khác) thì encoder và test cùng đổi theo, test vẫn xanh:

- **M5b** `DESC_ATTR_SHIFT 2 → 3` ⇒ **22 passed; 0 failed** (XANH).
- **M7b2** `DESC_NG 1<<11 → 1<<60` ⇒ **22 passed; 0 failed** (XANH).

Nghĩa là câu trong doc test "từng bit … **đúng đặc tả ARMv8-A**" chỉ đúng ở mức **tự nhất quán**,
không được neo vào **số bit literal** của ARMv8-A. (Một số vị trí khác bị bắt **gián tiếp** do
đè trường: `DESC_SH_SHIFT 8→7` đè `AP[2]`, `DESC_AF 10→14` đè trường địa chỉ — nhưng đó là may,
không phải thiết kế.) Đề nghị: thêm assert bằng **literal** (`1<<10`, `1<<11`, `0b11<<8`, …) hoặc
một bảng bit độc lập, để đột biến vị trí bit bị bắt.

### 3.4 Phát hiện T2 — `translate` bỏ kiểm bảng-mức-lá **không test nào bắt**

`translate` (`mmu.rs:355`) có nhánh `Descriptor::Table { pa } => { if level + 1 == F::LEVELS {
return Err(CorruptTable) } … }`. Xoá đúng kiểm này (M8) ⇒ **22 passed; 0 failed** (XANH). Không
test nào ghi một table descriptor vào **mức lá** để chốt `CorruptTable`; `block_descriptor_rejected`
chỉ ghi block ở mức 0/1. Vậy yêu cầu "translate bỏ kiểm cấp bảng ⇒ test phải ĐỎ" **không đạt** —
kiểm đó hiện là code phòng thủ **không có răng**.

### 3.5 Phát hiện T3 — crate **không** nằm trong `gate --fast`

`v2/bin/v2:416` chỉ chạy `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common`
cho workspace kernel. `vifast-plat-aarch64` là member mới (`v2/kernel/Cargo.toml`) nhưng **không**
được test/build trong `v2 check` hay `gate --fast`. Hệ quả: 22 test và cả việc crate **có biên
dịch được** chỉ được kiểm khi ai đó gõ tay đúng lệnh; một lỗi biên dịch trong crate này sẽ **lọt
cổng**. Đề nghị thêm `-p vifast-plat-aarch64` (hoặc `--workspace` cho kernel) vào L0.

### 3.6 Kết luận KT-160

**ĐẠT CÓ ĐIỀU KIỆN.** Crate/deliverable có thật, build thật, 22 test xanh, các đột biến **logic
encoder** đều đỏ. Điều kiện: (i) neo test descriptor vào bit literal (T1); (ii) thêm ca âm cho
`translate` gặp table ở mức lá (T2); (iii) đưa crate vào L0 của cổng (T3). Chưa tuyên bố K4' đạt
— đúng như tác giả đã ghi.

---

## 4. Đối chiếu nhanh với lời bàn giao

| Lời bàn giao | Đo lại | Kết |
|---|---|---|
| KT-158: `used=4`, `d docs`, `f note.txt` | có | đúng |
| KT-158: ca 16 14 phép kiểm, ảnh sạch 14/14 | có | đúng |
| KT-158: đột biến `pwd` hằng `/` ⇒ ĐỎ | có | đúng |
| KT-159: `--selftest` 15/15 | có | đúng |
| KT-159: 6 ca ghim xanh | có | đúng |
| KT-159: 2 đột biến ngữ nghĩa ĐỎ | có (2 failed mỗi đột biến) | đúng |
| KT-159: "cưỡng chế bằng máy" | lách được 6 cách, scope thiếu crate sản phẩm | **chưa đủ** |
| KT-160: 22 test host | có | đúng |
| KT-160: build `aarch64-unknown-none` | có `Compiling` + probe | đúng |
| KT-160: trait không có ở crate chung | grep rỗng | đúng |
| KT-160: 3 đột biến tác giả ĐỎ | không chạy lại đúng 3 đột biến đó; chạy 4 đột biến **khác** đều ĐỎ | tương đương |
| KT-160: test "đúng đặc tả ARMv8-A" | tautology vị trí bit (M5b/M7b2 xanh) | **chưa đủ** |

---

## 5. Giới hạn của review này

- **Không** chạy `gate --fast` (luật task). Có chạy `./v2/bin/v2 check` trên cây sạch — xem
  `kt161-logs/` / mục 6.
- **Không** chạy lại đúng ba đột biến của tác giả KT-160 (bỏ `AF`, `unmap` không `free_frame`,
  đảo `AP[2]`); thay bằng bốn đột biến **khác** (M5a/M6a/M7a/M10) — cùng lớp, đều ĐỎ.
- **Không** boot aarch64: `qemu-system-aarch64` **có** trên máy, nhưng crate KT-160 là **thư viện**
  (`no_std`, không `_start`, không ảnh boot) và tác giả không tuyên bố boot — boot sẽ là bước 2,
  ngoài phạm vi D2-147. Không có ảnh aarch64 nào để boot.
- **Không** kiểm persistence của `/docs` qua hai lần boot (ảnh sản phẩm là ram-only,
  `why=khong-co-pci`).
- Mọi đột biến đã `git checkout --`; cây sạch trước commit. QEMU của reviewer: **0** tiến trình
  còn sống (`pgrep -c qemu-system` = 0 sau mỗi lượt).

## 6. `./v2/bin/v2 check` trên cây sạch

```
$ ./v2/bin/v2 check
--- wake paths (KT-151: một cơ chế wake): OK
--- selftest wake paths (KT-151): OK
--- wake paths --strict (KT-155: allowlist): OK
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
```

`V2CHECK_RC=0` (`kt161-logs/v2-check-clean.log`). Lưu ý: `v2 check` **không** build/test
`vifast-plat-aarch64` (xem T3) — PASS ở đây không chứng minh crate KT-160 biên dịch; bằng chứng
cho việc đó là mục 3.2.
