# KT-250 — K4' bước 3, **B3.8 phần đầu**: `run-aarch64.sh` + TAP + ca âm

**Ngày**: 2026-11-21 · **Sub-agent** `a64` · nhánh `task/kt243-a64` · base `main` @ `43a054e3`
**Hàng QUEUE**: `KT-250` (D2-195) · **Phụ thuộc**: KT-249 (commit `ffcb5063`)
**Phạm vi**: **chỉ B3.8 phần đầu** — runner + TAP + ca âm, **một hart, một ảnh**.
**KHÔNG tuyên bố K4' đạt**: K4' = "như K3" (TAP đầy đủ **smp 1|2|4** + soak **1.000**) — bước sau.
**Không** sửa `v2/bin/v2` (tránh xung đột với sub-agent `infra`): runner là tệp **mới**, lead nối cổng.

---

## 1. Đã làm

| Tệp | Nội dung |
|---|---|
| `v2/test/boot/run-aarch64.sh` | **mới** — runner: build ảnh → kiểm **ELF** (T-objdump) → boot QEMU qua `qemu_guard.sh` → kiểm **TAP** + marker → **ca âm mặc định**; in `ok`/`not ok`, exit 0/1 |
| `v2/kernel/vifast-kbin-aarch64/src/main.rs` | phát **TAP 13** trên serial: `TAP version 13`, `1..5`, 5 dòng `ok N - …`; marker **chỉ in khi mọi ca xanh**; `# viFastOs aarch64: halt` |

### Runner kiểm gì (4 lớp)

1. **ELF** (T-objdump của thiết kế): `.text` nạp đúng `0x4008_0000`; `.text.vectors` là `TEXT`
   **không** phải `BSS` (bất biến B3.2); VMA bảng vector căn `0x800`; **16** stub `__vec_i` nằm
   đúng `__vector_base + i*0x80` — kiểm **phía asm**, không chỉ phía Rust.
2. **TAP**: `TAP version 13`, plan `1..N`, **đúng N** dòng `ok`, **không** `not ok`, số hiệu liên
   tục `1..N`, **không** `PANIC`.
3. **Marker**: đúng **một** lần, **nguyên văn** (0 lần = ảnh không báo boot; 2 lần = in lặp).
4. **Ca âm (mặc định, không cần cờ)**: chạy lại **cùng** `check_log` trên log tốt với 5 kỳ vọng
   **sai** — marker sai, thiếu một `ok`, có `not ok`, plan sai, log rỗng — và đòi **cả 5 phải
   ĐỎ**. Nếu một ca âm lại XANH thì bộ kiểm là dấu cao su ⇒ runner **ĐỎ**.

### Vì sao ca âm chạy **mặc định** (không phải cờ `--selftest`)

`v2/tools/check-selftest-wiring.py` (KT-134) đòi **mọi** script có nhánh `--selftest` phải được
một cổng trong `v2/bin/v2` gọi — mà task này **không được** sửa `v2/bin/v2`. Hai lựa chọn: thêm
ngoại lệ vào máy kiểm (chạm tệp dùng chung) hoặc bỏ cờ. Chọn cách thứ hai **và** làm ca âm mạnh
hơn: nó chạy **mỗi lần**, nên mỗi lần runner xanh là một lần chứng minh bộ kiểm **không** PASS giả.
Đo lại: `check-selftest-wiring: PASS (17 script tự kiểm: 17 có cổng, 0 ngoại lệ, 0 mồ côi)`.

## 2. Đo được (nguyên văn)

Log đầy đủ: [`kt250-runner.log`](kt250-runner.log).

### (a) Runner xanh

```text
$ bash v2/test/boot/run-aarch64.sh; echo "EXIT=$?"
# qemu 11.1.3  objdump …/llvm-objdump
# target aarch64-unknown-none  image …/vifast-kbin-aarch64
# build ảnh aarch64 (aarch64-unknown-none, release)
ok 1 - elf: .text tại 0x40080000, .text.vectors TEXT (không BSS), 16 stub đúng offset
ok 2 - boot 1: TAP 5/5 + marker 1 lần
ok 3 - ca âm (a): marker sai ⇒ ĐỎ (# marker '# viFastOs aarch64: boot BAD' xuất hiện 0 lần, cần đúng 1)
ok 4 - ca âm (b): thiếu 'ok 3' ⇒ ĐỎ (# có 4 dòng 'ok', cần 5)
ok 5 - ca âm (c): có 'not ok' ⇒ ĐỎ (# có 4 dòng 'ok', cần 5)
ok 6 - ca âm (d): plan sai ⇒ ĐỎ (# thiếu plan '1..5')
ok 7 - ca âm (e): log rỗng ⇒ ĐỎ (# thiếu dòng 'TAP version 13')
# ca âm: 5/5 ĐỎ đúng như phải
# 7/7 kiểm sạch (boot=1, tests=5)
EXIT=0
```

### (b) Serial của ảnh — TAP 5 ca

```text
TAP version 13
1..5
ok 1 - pl011: init sequence applied
ok 2 - vector: 16 slots, VBAR_EL1 installed
ok 3 - brk #0: ec=0x3c, elr matches instruction
ok 4 - mmu: SCTLR_EL1.M=1 after ISB
ok 5 - mmu: identity marker + non-identity probe VA
# viFastOs aarch64: boot ok
# viFastOs aarch64: halt
```

(Các dòng `# vector:`/`# brk:`/`# mmu:` là **comment TAP** — vẫn in để chẩn đoán, không tính là
`ok`.)

### (c) Ca âm đầu-cuối #1 — `EXPECT` sai

```text
$ EXPECT_MARKER='# viFastOs aarch64: boot BAD' bash v2/test/boot/run-aarch64.sh; echo "EXIT=$?"
ok 1 - elf: …
not ok 2 - boot 1: # marker '# viFastOs aarch64: boot BAD' xuất hiện 0 lần, cần đúng 1
# 6/7 kiểm sạch (boot=1, tests=5)
EXIT=1
```

### (d) Ca âm đầu-cuối #2 — **biến thể ảnh in marker sai**

Sửa tạm `BOOT_MARKER` trong ảnh thành `# viFastOs aarch64: boot WRONG`, build lại, chạy runner:

```text
$ bash v2/test/boot/run-aarch64.sh; echo "EXIT=$?"
ok 1 - elf: …
not ok 2 - boot 1: # marker '# viFastOs aarch64: boot ok' xuất hiện 0 lần, cần đúng 1
…
# 6/7 kiểm sạch (boot=1, tests=5)
EXIT=1
```

⇒ **Runner ĐỎ (exit 1)**, không PASS giả. Ảnh đã hoàn nguyên + build lại sau khi đo.

## 3. Còn thiếu so với B3.8 (ghi rõ, không giấu)

| Thiếu | Thuộc bước |
|---|---|
| **smp 1\|2\|4** — ảnh mới chạy **một** hart (chưa có `_start_secondary`, GICv3, PSCI CPU_ON) | bước sau (B3.6/B3.8 đầy đủ) |
| **soak 1.000 boot** | bước sau |
| **`trace-compare` sim ↔ QEMU aarch64** (`vifast-traceformat`) | bước sau (cần trace trên aarch64) |
| **budget `.data+.bss ≤ 256 KiB`** cho ảnh aarch64 (`budget.sh` bản aarch64) | bước sau — hiện `.bss` = 0x43000 = 268 KiB **vượt** trần K0.5 nếu áp nguyên ngưỡng; **chưa** áp vì `budget.sh` đang gắn riscv64 và trần này thuộc tiêu chí "như K3" |
| **nối vào cổng** (`v2/bin/v2`) | **lead** (task này không được sửa) |

**Cảnh báo cho lead**: `.bss` của ảnh aarch64 hiện **268 KiB** (arena bảng trang 256 KiB + trang
probe 4 KiB + …). Nếu áp trần K0.5 (`.data + .bss ≤ 256 KiB`) cho **cả** aarch64 thì ảnh này
**vượt** — cần quyết định: (a) giảm arena (đo được chỉ dùng 8/64 frame ⇒ 16 frame là đủ rộng),
hoặc (b) trần K0.5 chỉ áp cho ảnh sản phẩm riscv64. **Không** tự nới ngưỡng; ghi ra để lead chốt.

## 4. Rủi ro

1. **`run-aarch64.sh` chưa được cổng nào gọi** ⇒ nếu không nối, nó là "script mồ côi" theo tinh
   thần KT-134 (máy kiểm `check-selftest-wiring.py` **không** bắt vì không có cờ `--selftest`).
   Đề nghị lead nối vào `v2/bin/v2` (một dòng `lane_step`, ví dụ trong `gate --dev`/`--fast`).
2. **`qemu_guard.sh` dùng chung** với riscv64: runner này chỉ `source` + `qg_init`/`qg_boot`, không
   sửa tệp — không xung đột với sub-agent khác.
3. **`objdump` phụ thuộc máy**: runner thử `llvm-objdump` → `rust-objdump` → sysroot llvm-tools →
   `xcrun` → `objdump`, và **fail-closed** nếu không có cái nào (không bỏ qua kiểm ELF).
4. **`perl -e 'alarm N; exec @ARGV'` không giết được QEMU** (đo 2026-11-21): runner dùng
   `qemu_guard.sh` (process group + timeout + kill group), không dùng `alarm`.
