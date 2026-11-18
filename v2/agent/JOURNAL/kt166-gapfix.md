# KT-166 — vá ba lỗ hổng bằng chứng do reviewer L6 phát hiện (KT-158 + KT-160)

**Ngày**: 2026-11-19 · **Agent 2 (L9)** · nhánh `night2/L9-gapfix` · HEAD trước task: `e022cdbe`
**Nguồn nợ**: [`kt161-review.md`](kt161-review.md) §1.4 (M2 — `resolve_path` bỏ `Cwd` vẫn XANH),
§3.3 (T1 — test descriptor tự so hằng của crate), §3.4 (T2 — `translate` bỏ kiểm bảng-mức-lá
không test nào bắt), §3.5 (T3 — crate `vifast-plat-aarch64` không nằm trong `gate --fast`).

## 0. Việc được giao (nguyên văn, trích)

> **A. KT-158** — thêm vào ca 16 `shell-edge.sh`: sau `cd /docs`, chạy `cat note.txt` (đường
> **tương đối** trong thư mục con) ⇒ phải in `# shell: cat ok bytes=16 first=ghi chu`; và
> `stat note.txt` ⇒ `kind=1`; và `ls .` trong `/docs` ⇒ `n=1 path=/docs`. Cập nhật **đếm** trong
> `--selftest`. **Đột biến bắt buộc**: `resolve_path` bỏ qua `Cwd` ⇒ ca 16 **phải ĐỎ**.
>
> **B. KT-160** — thêm test dùng **hằng độc lập viết tay** cho các bit trường (`AttrIndx` bit
> 2..4, `AP[2:1]` bit 6..7, `SH` bit 8..9, `AF` bit 10, `nG` bit 11, `UXN` bit 54, `PXN` bit 53),
> khẳng định **giá trị tuyệt đối** của descriptor. Thêm test `translate` gặp descriptor **bảng**
> ở mức lá ⇒ phải trả lỗi. **Đột biến bắt buộc**: `DESC_ATTR_SHIFT 2→3` ⇒ ĐỎ; `DESC_NG 1<<11→1<<60`
> ⇒ ĐỎ; xoá kiểm table-at-leaf ⇒ ĐỎ. Thêm `-p vifast-plat-aarch64` vào L0 kernel workspace.

## 1. Kết luận trước (đọc nhanh)

- **A. KT-158 — ĐÓNG.** Ca 16 có **15** phép kiểm (14 → 15), thêm ba lệnh **tương đối trong**
  `/docs`: `cat note.txt`, `stat note.txt`, `ls .`. Ảnh sạch **15/15 PASS**; đột biến
  "`resolve_path` bỏ qua `Cwd`" ⇒ **4 phép kiểm ĐỎ** (`ls-cwd`, `ls-docs`, `cat-docs`,
  `stat-docs`) + ca 16 ĐỎ + `shell-edge.sh: FAIL`. Đúng lỗ hổng reviewer L6 dựng lại.
- **B. KT-160 — ĐÓNG.** Thêm `descriptor_bits_literal_armv8a` (bảng bit **viết tay**, khẳng định
  **giá trị tuyệt đối** `0x0060_0000_8000_1707` cho leaf KERNEL_RW và `0x0020_0000_8000_1FC7`
  cho leaf USER_RX) + `translate_table_at_leaf_rejected`. Ba đột biến bắt buộc đều **ĐỎ**;
  baseline **24 passed** (22 → 24).
- **C. Cổng — ĐÓNG.** `v2/bin/v2` L0 kernel workspace nay chạy
  `cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common -p vifast-plat-aarch64`
  (cargo nhận **hai** `-p` cùng lúc — đã đo).
- `./v2/bin/v2 check` ⇒ **PASS**; `shell-edge.sh --selftest` ⇒ **42/42** (9/9 ca).

## 2. A — KT-158: đường tương đối **trong** thư mục con

### 2.1 Vì sao trước KT-166 vẫn xanh

Reviewer L6 (KT-161 §1.4) sửa `resolve_path` (`shell_user.rs:464`) cho nhánh đường **tương đối**
luôn bắt đầu từ `/` (bỏ `cwd`), giữ nguyên `cd` cập nhật `Cwd` và `pwd` in `cwd`. Kết quả:
**ca 16 vẫn XANH toàn bộ**. Nguyên nhân: khối `/docs` của KT-158 chỉ dùng **đường tuyệt đối**
(`cat /docs/note.txt`, `cd /docs/khongco`, `cd /docs/..`) và `ls` **không tham số** (đi thẳng
`cwd.as_slice()`, **không** qua `resolve_path`); lệnh tương đối duy nhất chạy trong `/docs` là
`cd ..` — ở `/docs` hay ở `/` nó đều in `# shell: cd /`. Toàn bộ đường tương đối còn lại chạy khi
`cwd == /`, nơi "bỏ qua `Cwd`" trùng hành vi đúng.

### 2.2 Đã thêm (ca 16, sau `cat /docs/note.txt`)

```text
cd /docs
pwd
ls
cat /docs/note.txt
cat note.txt        ← KT-166: tương đối TRONG /docs
stat note.txt       ← KT-166
ls .                ← KT-166
cd /docs/khongco
...
```

Ba phép kiểm tương ứng (đếm **toàn log**, không nới ngưỡng — chỉ **siết**):

| Phép kiểm | Trước | Sau | Bắt gì |
|---|---|---|---|
| `cat-docs` | 1 dòng nội dung + 1 dòng `cat ok` | **2 + 2** (tuyệt đối + tương đối) | `cat note.txt` không phân giải ⇒ còn 1 |
| `ls-docs` | 1 dòng `ls n=1 path=/docs` + `f note.txt` ≥ 1 | **2 + 2** (`ls` + `ls .`) | `ls .` không phân giải ⇒ còn 1 |
| `stat-docs` (**mới**) | — | **1** dòng `stat path=/docs/note.txt kind=1 size=16 ino=<n>` | `stat note.txt` không phân giải ⇒ 0 |

`check_kt137_cdpwd` nay có **15** cặp phán quyết; `--selftest` cập nhật: log sai **2 PASS + 13
FAIL**, log đúng **15 PASS + 0 FAIL** (tổng 42/42 kiểm không đổi — `selftest_verdicts` đếm **lời
gọi**, không đếm phán quyết).

### 2.3 Bằng chứng — ảnh sạch (15/15)

```
$ bash v2/tools/repro-build.sh --target-dir /tmp/l9-target
REPRO … sha256=85a80b429729e802b07329725a3045910fbb7f61d953941992021058475b2bda size=213632 dirty=0 rev=e0ab558956b1 …
$ bash v2/test/boot/shell-edge.sh --image /tmp/l9-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
# kt137-cdpwd[pwd]: PASS
# kt137-cdpwd[cd-khong-co]: PASS
# kt137-cdpwd[cd-tep]: PASS
# kt137-cdpwd[cd-thu-tu]: PASS
# kt137-cdpwd[cat-tuong-doi]: PASS
# kt137-cdpwd[stat-tuong-doi]: PASS
# kt137-cdpwd[write-tuong-doi]: PASS
# kt137-cdpwd[cat-rel]: PASS
# kt137-cdpwd[truncate-tuong-doi]: PASS
# kt137-cdpwd[stat-rel]: PASS
# kt137-cdpwd[ls-cwd]: PASS
# kt137-cdpwd[cd-docs]: PASS
# kt137-cdpwd[ls-docs]: PASS
# kt137-cdpwd[cat-docs]: PASS
# kt137-cdpwd[stat-docs]: PASS
# kt137-cdpwd: PASS (# shell: pwd /)
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 + new-cmds-edge3 + k7-uu-tien + k7-thu-muc + kt137-cdpwd PASS
# shell-edge.sh: PASS
```

**Ghi chú**: dòng `REPRO` trên là log **nguyên văn** lúc dựng ảnh (commit `e0ab5589`). Commit
cuối chỉ chạm **tệp nhật ký này** — `v2/kernel`/`v2/lib` không đổi — nên ảnh dựng lại từ commit
cuối là **byte-identical** (cùng `inputs=efe45ec08a656d39`, cùng `text=131084`).

Lát log `/docs` thật (`/tmp/l9-logs/run-clean-final/qemu.log:296-313`):

```
# shell: cd /docs
# shell: pwd /docs
  f note.txt
# shell: ls n=1 path=/docs
ghi chu cua docs
# shell: cat ok bytes=16 first=ghi chu 
ghi chu cua docs
# shell: cat ok bytes=16 first=ghi chu 
# shell: stat path=/docs/note.txt kind=1 size=16 ino=4
  f note.txt
# shell: ls n=1 path=/docs
```

### 2.4 Bằng chứng — đột biến `resolve_path` bỏ `Cwd` (ĐỎ)

Sửa `resolve_path` (`shell_user.rs:464`): nhánh tương đối `out[0] = b'/'; len = 1;` (bỏ
`cwd.len`/`cwd.as_slice()`). Ảnh `sha256=51dc95edd1df8140702657891426cfd3f6fa27e77ff9aa77f8606de72e2ae5a4`:

```
# kt137-cdpwd[ls-cwd]: FAIL (mong đúng 2 dòng '# shell: ls n=<n> path=/' (ls + ls .), thấy 3)
# kt137-cdpwd[ls-docs]: FAIL (mong đúng 2 dòng '# shell: ls n=1 path=/docs' (ls + ls .), thấy 1)
# kt137-cdpwd[cat-docs]: FAIL (mong đúng 2 dòng nội dung 'ghi chu cua docs' (cat /docs/note.txt + cat note.txt), thấy 1)
# kt137-cdpwd[stat-docs]: FAIL (mong đúng 1 dòng '# shell: stat path=/docs/note.txt kind=1 size=16 ino=<n>', thấy 0)
# kt137-cdpwd: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)
# shell-edge: có ca ĐỎ (xem log /tmp/l9-logs/run-mut/qemu.log)
# shell-edge.sh: FAIL
```

Lát log `/docs` dưới đột biến (`/tmp/l9-logs/run-mut/qemu.log:296-312`) — `cat note.txt` và
`stat note.txt` thành `/note.txt` ⇒ `fs loi code=5`; `ls .` thành `ls /`:

```
# shell: cd /docs
# shell: pwd /docs
  f note.txt
# shell: ls n=1 path=/docs
ghi chu cua docs
# shell: cat ok bytes=16 first=ghi chu 
# shell: fs loi code=5
# shell: fs loi code=5
  f README
  d docs
  f a
```

⇒ **Đột biến của reviewer L6 nay bị bắt.** Gỡ đột biến (`git checkout -- …/shell_user.rs`) ⇒
ảnh sạch 15/15 xanh trở lại (mục 2.3).

## 3. B — KT-160: neo bit vào đặc tả + ca âm bảng-mức-lá

### 3.1 T1 — test descriptor là tautology một phần

`descriptor_bits_aarch64` (`host.rs:181`) và `permissions_roundtrip` (`:282`) kiểm bit bằng
**chính** các hằng `DESC_*` mà `encode_leaf` dùng. Đổi **vị trí** một bit (không đè trường khác)
thì encoder và test cùng đổi theo ⇒ **22/22 vẫn xanh** (L6: `DESC_ATTR_SHIFT 2→3`,
`DESC_NG 1<<11→1<<60`).

**Đã thêm** `descriptor_bits_literal_armv8a` + khối `mod armv8a` — vị trí bit **chép tay từ
ARMv8-A** (ARM DDI 0487, D8.3), **không** lấy từ crate:

| Trường | Bit (viết tay) |
|---|---|
| `AttrIndx[2:0]` | 4:2 |
| `AP[2:1]` | 7:6 |
| `SH[1:0]` | 9:8 |
| `AF` | 10 |
| `nG` | 11 |
| `PXN` | 53 |
| `UXN` | 54 |
| PA | 47:12 |

Khẳng định **giá trị tuyệt đối** cho leaf đã biết (VA `0x1000`, PA `0x8000_1000`,
`Perms::KERNEL_RW`, Normal, Inner):

```rust
assert_eq!(d, 0x0060_0000_8000_1707, "giá trị tuyệt đối của leaf KERNEL_RW (tính tay từ đặc tả)");
```

và leaf user RX (`Perms::USER_RX` — có `nG`, nên bắt được đột biến `DESC_NG`):

```rust
assert_eq!(u, 0x0020_0000_8000_1FC7, "giá trị tuyệt đối của leaf USER_RX (tính tay từ đặc tả)");
```

### 3.2 T2 — `translate` bỏ kiểm bảng-mức-lá không test nào bắt

`translate` (`mmu.rs:339`) có nhánh `Descriptor::Table { pa } => { if level + 1 == F::LEVELS {
return Err(CorruptTable) } … }`. L6 xoá đúng kiểm này ⇒ **22/22 xanh**.

**Đã thêm** `translate_table_at_leaf_rejected`. Ghi chú kỹ thuật **quan trọng**: AArch64
`decode` **không bao giờ** trả `Table` ở mức lá — `0b11` ở mức lá là **page descriptor** (đúng
ARMv8-A), nên nhánh phòng thủ **không chạm được** qua `Aarch64`. Vì vậy ca âm dùng format
`TableAtLeaf4` **cố ý hỏng** (4 mức, `decode` trả `Table` ở mọi mức) — đúng loại dữ liệu hỏng mà
nhánh ấy sinh ra để chặn. Test dựng đường đi 4 mức bằng descriptor **bảng** ở cả ba mức trung
gian, rồi đặt descriptor **bảng** vào **mức lá** (level 3) và đòi `CorruptTable`:

```rust
assert_eq!(
    mmu.translate(VA),
    Err(MapError::CorruptTable),
    "bảng ở mức lá phải CorruptTable, không được trả PA rác"
);
```

### 3.3 Bằng chứng — ba đột biến bắt buộc (đều ĐỎ)

```
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

# M1: DESC_ATTR_SHIFT 2 → 3
test descriptor_bits_literal_armv8a ... FAILED
thread 'descriptor_bits_literal_armv8a' panicked at vifast-plat-aarch64/tests/host.rs:304:5:
test result: FAILED. 23 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

# M2: DESC_NG 1<<11 → 1<<60
test descriptor_bits_literal_armv8a ... FAILED
thread 'descriptor_bits_literal_armv8a' panicked at vifast-plat-aarch64/tests/host.rs:344:5:
test result: FAILED. 23 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s

# M3: xoá `if level + 1 == F::LEVELS { return Err(CorruptTable) }` trong translate
test translate_table_at_leaf_rejected ... FAILED
thread 'translate_table_at_leaf_rejected' panicked at vifast-plat-aarch64/tests/host.rs:778:5:
test result: FAILED. 23 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

Gỡ cả ba ⇒ `24 passed; 0 failed` (log `/tmp/l9-logs/final-*.log`).

### 3.4 T3 — crate không nằm trong cổng

`v2/bin/v2:427` (trước KT-166) chỉ chạy `-p vifast-plat-common` cho workspace kernel. Nay:

```sh
run_step "L0 kernel workspace (KT-148)" "$CARGO" test --manifest-path "$V2_ROOT/kernel/Cargo.toml" -p vifast-plat-common -p vifast-plat-aarch64 || return 1
```

Cargo nhận **hai** `-p` cùng lúc (đã đo — không cần hai lệnh):

```
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common -p vifast-plat-aarch64
     Running tests/host.rs (…/host-274a1f589a1f4d17)
test result: ok. 24 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
     Running tests/fault.rs (…/fault-ca7cfc1e04b56d7e)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     … (vifast-plat-common: fault 6, host 14, ipi 12, mmu 8, mmu_shared 3, obj 15, plic 12,
        resources 7, smp 9, timer 12, tlb 11, user 9 — tất cả `ok`)
```

## 4. Cổng

```
$ ./v2/bin/v2 check
…
--- test có răng (KT-163): OK
--- selftest test có răng (KT-163): OK
v2 check: PASS (L1 chạy trong gate --fast)
```

`shell-edge.sh --selftest` (chạy trong `v2 check`):

```
# selftest 16 kt137-cdpwd (sai): ĐÚNG (2 PASS + 13 FAIL, mã thoát 1)
# selftest 16 kt137-cdpwd (dung): ĐÚNG (15 PASS + 0 FAIL, mã thoát 0)
# shell-edge-selftest: PASS (9/9 ca, 42/42 kiểm: 23 sai→ĐỎ + 19 đúng→XANH)
```

## 5. Tệp đã sửa

| Tệp | Thay đổi |
|---|---|
| `v2/test/boot/shell-edge.sh` | ca 16: +3 lệnh tương đối trong `/docs`; `cat-docs`/`ls-docs` siết thành **2**; +`stat-docs` (15 phép kiểm); selftest log giả + đếm; header |
| `v2/kernel/vifast-plat-aarch64/tests/host.rs` | +`mod armv8a` (bit viết tay) + `descriptor_bits_literal_armv8a`; +`TableAtLeaf4` + `translate_table_at_leaf_rejected` (22 → 24 test) |
| `v2/bin/v2` | L0 kernel workspace: `-p vifast-plat-common -p vifast-plat-aarch64` |

**Không** sửa `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`,
`docs/v1/`. Không nới ngưỡng/checker, không `#[ignore]`/`#[should_panic]`, không retry.

## 6. Còn thiếu / ghi chú

- **`gate --fast` không chạy** (luật task cấm). Bước L0 kernel workspace mới được kiểm bằng cách
  chạy **đúng lệnh cargo** của nó (mục 3.4) + `v2 check` PASS; chưa có log `gate --fast` đầy đủ.
- **Nhận xét nhẹ của L6 (KT-161 §1.5) chưa xử lý**: các phép kiểm `/docs` vẫn grep **toàn log**
  của ca thay vì lát log `/docs`. Hôm nay không có dòng nào khác sinh cùng chuỗi nên không
  false-positive; muốn siết thì cần cắt lát log — **không** làm trong task này (ngoài phạm vi,
  và cắt lát làm tăng rủi ro harness).
- **Ca âm bảng-mức-lá dùng format cố ý hỏng** (`TableAtLeaf4`), không phải AArch64 — vì
  `Aarch64::decode` không bao giờ trả `Table` ở mức lá (đúng đặc tả). Nhánh phòng thủ là **generic
  trên `F`**, nên test này chạm đúng đoạn mã cần chặn; đột biến xoá nhánh ⇒ ĐỎ (đã đo).
- **QEMU**: chỉ mở **1** QEMU mỗi lượt (ca 16), `pgrep -c qemu-system` = 0 trước khi mở và sau khi
  đóng; không `pkill -f qemu-system`.
