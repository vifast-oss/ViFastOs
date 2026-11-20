# KT-265 — aarch64 **soak 1.000** + `budget-aarch64.sh` (điều kiện "như K3" của K4')

Trạng thái: **DONE** (nhánh `task/kt265-a64soak`, base `main` @ `223d4f74`), 2026-11-21.
Sub-agent `a64soak`. Quyết định: **D2-204** (nhiệm vụ), **D2-195** (phạm vi bước 3), **D2-198**
(uỷ quyền commit trên nhánh task), **D2-24/D2-72 Q10/D2-74/D2-80** (bất biến soak).

**Không** tuyên bố K4' đạt: K4' = "**như K3**" (TAP đầy đủ smp 1|2|4 **+ soak 1.000**). KT-265 làm
phần **soak 1.000** + **budget aarch64** + dòng **`# conf image:`**; các mục còn lại của K4' vẫn
mở (xem §7).

## 1. Phạm vi đã làm

| Việc | Kết quả |
|---|---|
| `v2/test/boot/soak-aarch64.sh` (khuôn `soak.sh`, **cùng bất biến**) | **DONE** — N=1.000, 3 luồng, 20 s/boot, không retry, `--smoke`, `--selftest` |
| `v2/test/boot/budget-aarch64.sh` (trần 262.144 B, `.smp_stacks` báo riêng) | **DONE** — `--selftest` 8 ca |
| Ảnh aarch64 in `# conf image: build=<16 hex> text=<len>` (D2-80) | **DONE** — `vifast_plat_aarch64::image` (FNV-1a 64 trên `.text` đã nạp) |
| `run-aarch64.sh` kiểm hash ảnh + ca âm (j) | **DONE** — 12/12 kiểm, **10** ca âm |
| **Soak thật 1.000** | **DONE** — **0/1000 lỗi**, wall 247 s (xem §3) |

## 2. Test đỏ trước (D2-05)

Thứ tự làm đúng luật: **viết script trước, chạy nó trên ảnh CHƯA có dòng hash ⇒ ĐỎ**, rồi mới sửa
kernel.

```text
$ bash v2/test/boot/soak-aarch64.sh --smoke 3 3        # ảnh chưa in '# conf image:'
# probe (D2-80): boot MỘT lần để đọc dòng '# conf image:' của ảnh
# soak-aarch64.sh: TỪ CHỐI ĐO — ảnh in '# conf image: build=none text=none', ELF là
#                  build=30fc83f81d5245ee text=37044 (D2-80)
# soak-aarch64.sh: ảnh KHÔNG tự khai hash khớp ELF ⇒ không có neo, không đo
exit=3
```

Sau khi thêm dòng hash vào kernel: `--smoke 3 3` ⇒ `ok=3 fail=0`, exit 1 (đúng: smoke **không**
phải bằng chứng cổng). Đó là **cùng** một script, **cùng** một bộ kiểm — không có đường thứ hai.

## 3. Soak thật — **0/1000**

```text
$ bash v2/test/boot/soak-aarch64.sh 1000 3
# qemu 11.1.3 (sàn 9.2, D2-72 Q2)
# cấu hình: -M virt,gic-version=3 -cpu cortex-a72 -m 128M -nographic -smp 4 -no-reboot; KHÔNG icount
# N=1000 lanes=3 timeout=20s/boot; p=3/1000 (0,3 %) ⇒ n ≥ 3/p = 1000 (D2-24)
# git HEAD=223d4f74e19374370dc108009694070e0ed311dc dirty-kernel-paths=5
# image=product bytes=149328 sha256=b7568e134ebb74263599aaa22402aba62714dde4649cc6ee3e7b6e95a0edf3ae
# BUILD_ID (D2-80) = build=68b890daa9e08f6b text=38072
# probe: '# conf image:' khớp ELF (build=68b890daa9e08f6b text=38072)
# soak: xong wall=247s (4 boot/s); ok=1000 fail=0 tổng=1000/1000
# soak: 0/1000 lỗi (PASS)
# soak-aarch64.sh: PASS
exit=0
```

| Phép đo | Kết quả |
|---|---|
| N / luồng / timeout | **1.000** / **3** / **20 s** mỗi boot (không nới) |
| boot đỏ | **0** — `results.txt`: 1000 dòng `ok`, **0** dòng `fail` |
| wall | **247 s** (≈ 4 boot/s) — **thời gian bị đánh dấu KHÔNG HỢP LỆ**, xem dưới |
| guard (`qemu_guard.sh`) | `boot_end=1001 isolated_1=1001 isolated_0=0 rc_0=1001 rc_124=0` (1000 soak + 1 probe) |
| ảnh đổi giữa soak | **không** (sha256 đầu == cuối) |
| retry | **0** (D2-07) |

**Cảnh báo hạ tầng — ghi trung thực:** mẫu `qg_infra after` báo
`qemu_total=1 qemu_own=0 qemu_foreign=1 ⇒ verdict=KHONG_HOP_LE` ⇒
`# MEASUREMENT_INVALID tag=after`. Truy nguyên: đó là **`qemu-system-x86_64` của sub-agent KT-266**
(worktree khác, `-kernel v2/kernel/target/x86_64-unknown-none/...`), **không** phải QEMU của soak
này (`qemu_own=0`, guard 1001/1001 `isolated=1 rc=0`). Hệ quả đúng theo D2-116/OD-K6-6: **số thời
gian** (`wall=247s`, `4 boot/s`) **không dùng để kết luận**; **số lỗi 0/1000 vẫn hợp lệ** vì mỗi
boot được kiểm **độc lập** (TAP + marker + hash + smp + tlbi) và **không** boot nào chạm timeout
(`rc_124=0`). Không chạy lại soak để "lấy số đẹp" (AGENTS §6.3).

Bất biến **mỗi** boot (fail-closed, `check_boot_log`):

1. đúng 1 `TAP version 13`, đúng 1 `1..15`, đúng **15** dòng `ok N - ` (đủ số 1..15), **0** `not ok`;
2. đúng 1 `# viFastOs aarch64: boot ok`;
3. đúng 1 `# conf image: build=<16 hex> text=<len>` **khớp ELF** (D2-80);
4. `# smp: online mask=0xf alive=0xf expect=0xf harts=4 mpidr_ok=1` + `# smp: hart=N online|alive`
   cho cả bốn hart + `# smp: ipi recv=1` đúng một lần + ba dòng `# tlbi:` (read1 `match=1`,
   shootdown `acks=3 ok=1`, read2 `fault=1`);
5. **0** dòng `PANIC` (tính cả dòng thụt đầu dòng);
6. quá 20 s ⇒ `kill` **cả process group**, boot đó đỏ, **không retry**.

`--selftest` (không mở QEMU): **1 ca dương + 13 ca âm**, tất cả đúng như phải — bộ đọc **không**
phải dấu cao su.

## 4. `budget-aarch64.sh` — số đo

```text
$ bash v2/test/boot/budget-aarch64.sh <ảnh> product
# image=product ... arch=aarch64(e_machine=183) data+bss=155872 budget=262144
#   stack=0 guard=0 smp_stacks=557056 noload=557056
# sections-seen: .text=38072 .text.vectors=2072 .rodata=11624 .data=208 .bss=155664 .smp_stacks=557056
# data+bss-sections: .data=208 .bss=155664
```

| Phép đo | Kết quả |
|---|---|
| `.data + .sdata + .bss + .sbss` | **155.872 B** / trần **262.144 B** (59,5 %) |
| `.smp_stacks` (NOLOAD per-hart, KT-259) | **557.056 B** — **báo riêng**, không cộng trần |
| `.stack` / `.stack_guard` | 0 / 0 (linker aarch64 dùng `.smp_stacks`; script vẫn báo nếu có) |
| cổng kiến trúc | `e_machine=183` (AArch64) — trỏ nhầm ảnh riscv64/x86_64 ⇒ **TỪ CHỐI ĐO** |
| `--selftest` | **8/8** (gồm ca `.bss` 300 KiB vượt trần, `.smp_stacks` 4 MiB **không** kéo tổng, tệp không phải ELF bị từ chối) |

## 5. Dòng `# conf image:` (D2-80) — thiết kế

- `vifast_plat_aarch64::image::image_digest()`: FNV-1a 64 trên `[__text_start, __text_end)` của
  **chính ảnh đang chạy**, đọc **từng byte** bằng `asm::read_u8` (đọc `u64` sẽ vượt `__text_end`
  ở đoạn đuôi — `text=38072` không chia hết 8 — và băm cả byte của `.text.vectors`).
- `report_image_digest()` in `# conf image: build=<16 hex> text=<len>` **ngay sau** `1..15`, trước
  mọi ca kiểm ⇒ log của một boot hỏng vẫn trả lời được "ảnh nào".
- **`unsafe` không tăng**: phép đọc nằm ở `asm::read_u8` (mô-đun duy nhất của crate được
  `#![allow(unsafe_code)]`, D2-17); `image.rs` **không** có `unsafe`; lấy địa chỉ symbol bằng
  `core::ptr::addr_of!` (an toàn).
- **Host**: `image_digest()` **panic** (fail-loud) — không có bản đồ kernel/symbol linker; test
  `host_stub_is_fail_loud` chốt điều đó. Phần **thuần** `fnv1a64` test với **5 vector chuẩn FNV**
  (`""`, `"a"`, `"foobar"`, `"hello"`, `"ViFastOs"`) ⇒ cùng hằng với `image_digest.py`.
- `run-aarch64.sh` băm lại **cùng** section từ ELF và đòi log khớp **từng trường**; ca âm (j) đột
  biến hash ⇒ ĐỎ.

## 6. Nợ P20 — sha ảnh phụ thuộc **cách viết đường dẫn** build (đo được, không sửa trong task này)

Cùng một cây, cùng lệnh, nhưng:

| Cách gọi | `build=` | sha256 (16 hex đầu) |
|---|---|---|
| `cd /tmp/kt264-s2 && cargo build --manifest-path v2/kernel/Cargo.toml …` | `31e033672c0648de` | `c5281b07264899f6` |
| `cd / && cargo build --manifest-path /tmp/kt264-s2/v2/kernel/Cargo.toml …` | `68b890daa9e08f6b` | `b7568e134ebb7426` |

Hai đường dẫn (`/tmp/...` và `/private/tmp/...`) tạo **hai bộ artifact** trong cùng `target/`
(metadata hash của cargo gồm chuỗi đường dẫn workspace); mỗi lần gọi, cargo chép bộ khớp fingerprint
ra tên ảnh cuối. `.text` khác nhau **3.707 byte** (cùng cỡ 38.072 B). Đây là **cùng lớp nợ P20**
(AGENTS §6.3: "sha256 ảnh phụ thuộc thư mục build") — **không** phải lỗi của soak: soak **tự build**
rồi **tự băm** ảnh trong **cùng một lần chạy**, nên `# conf image:` của kernel và bản băm từ ELF luôn
khớp; sha đầu == sha cuối. **Hệ quả cho người đọc:** không so sha ảnh giữa hai lần chạy khác cách
gọi; danh tính bền vẫn là `# conf image:` + `git HEAD` + patch sha.

## 7. Còn thiếu / rủi ro (ghi rõ, không giấu)

1. **K4' chưa đạt**: soak 1.000 **đã xong**, nhưng K4' còn U-mode (B3.7 — chặn bởi Q2:
   `spec/abi.toml [calling.aarch64]` chưa mở), timer per-hart/preemption, mỗi hart một address
   space/ASID, đọc DTB, W^X đầy đủ (`k4-aarch64-boot.md` §7).
2. **Ảnh aarch64 chưa in `# build:` (KT-86)** — cổng danh tính bản dựng của riscv64 chưa áp cho
   aarch64; bù bằng `# conf image:` + sha256 + `git HEAD` trong header soak. **Không** gọi đây là
   tương đương KT-86.
3. **Thời gian soak không hợp lệ** vì QEMU ngoài (KT-266) — xem §3. Số lỗi vẫn hợp lệ.
4. **Soak phải chạy độc quyền** (AGENTS §6: soak không chạy song song build/cổng khác). Lần này
   một QEMU x86_64 của sub-agent khác xuất hiện ở mẫu `after`; không ảnh hưởng 0/1000 nhưng làm
   số thời gian không dùng được.
5. **`budget-aarch64.sh` là script riêng** (không sửa `budget.sh` riscv64) — lead nối bước cổng
   nếu muốn; KT-265 **không** thêm bước vào `v2/bin/v2` (đúng nhiệm vụ).

## 8. Tệp đã đổi

| Tệp | Việc |
|---|---|
| `v2/test/boot/soak-aarch64.sh` | **mới** — soak 1.000 aarch64 (khuôn `soak.sh`) |
| `v2/test/boot/budget-aarch64.sh` | **mới** — ngân sách `.data+.bss` aarch64 |
| `v2/kernel/vifast-plat-aarch64/src/image.rs` | **mới** — FNV-1a 64 + `report_image_digest` |
| `v2/kernel/vifast-plat-aarch64/src/asm.rs` | `read_u8` (bare-metal + host stub) |
| `v2/kernel/vifast-plat-aarch64/src/lib.rs` | `pub mod image` + re-export |
| `v2/kernel/vifast-plat-aarch64/tests/image.rs` | **mới** — 5 test host (vector FNV + stub fail-loud) |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | gọi `report_image_digest()` sau `1..15` |
| `v2/test/boot/run-aarch64.sh` | `check_digest` + ca âm (j) + `WANT_BUILD/WANT_TEXT` |
| `v2/docs/k4-aarch64-boot.md` | §6/§7 cập nhật (soak, budget, hash ảnh) |
| `v2/agent/JOURNAL/kt265-*` | log bằng chứng (soak, results, guard, infra, boot mẫu) |

## 9. Bằng chứng trong journal

| Tệp | Nội dung |
|---|---|
| `kt265-soak-1000.log` | toàn bộ console của soak 1.000 (header + 1000 dòng `boot i/1000 ok` + tổng) |
| `kt265-soak-results.log` | `results.txt` của soak: 1000 dòng `ok i`, **0** dòng `fail` |
| `kt265-guard-summary.log` | `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0` |
| `kt265-infra.log` | mẫu `qg_infra before/after` (gồm cảnh báo `qemu_foreign=1`) |
| `kt265-boot-1.log` | log **một** boot đầy đủ (TAP 15/15 + `# conf image:` + `# smp:` + `# tlbi:`) |

## 10. Ràng buộc bằng chứng với commit

Soak chạy **trước** commit, trên cây có 5 tệp `v2/kernel` khác HEAD (header soak ghi
`git HEAD=223d4f74… dirty-kernel-paths=5`, `sha256 patch=3a505ec8…`). Commit **`44faebbb`** chứa
**đúng** nội dung cây đã soak. Build lại trên cây **sạch** (sau `cargo fmt`, sau commit):

```text
# BUILD_ID (D2-80) = build=68b890daa9e08f6b text=38072
sha256 = b7568e134ebb74263599aaa22402aba62714dde4649cc6ee3e7b6e95a0edf3ae
```

**Trùng byte** với ảnh đã soak (`build=68b890daa9e08f6b`, `sha256=b7568e13…`) ⇒ bằng chứng
**0/1000** áp đúng cho commit `44faebbb`; **không** chạy lại soak (cùng fingerprint — AGENTS §6.3
cấm chạy lại soak trên cùng fingerprint chỉ để tăng số lần PASS).
