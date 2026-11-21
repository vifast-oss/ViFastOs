# KT-291 — riscv64 **soak 1.000** trên ảnh hiện tại (sau vòng 13)

Trạng thái: **DONE** (nhánh `task/kt291-rvsoak2`, base `main` @ `36e709ce`), 2026-11-21.
Sub-agent `rvsoak2`. Quyết định: **D2-212** (nhiệm vụ soak vòng 12), **D2-213** (nhiệm vụ vòng 13),
**D2-198** (uỷ quyền commit trên nhánh task), **D2-24/D2-72 Q10/D2-74/D2-80** (bất biến soak),
**D2-116** (QEMU orphan / `MEASUREMENT_INVALID`), **AGENTS §6.3** (tái dùng bằng chứng).

**Ghi chú đọc trước**: tại **base `36e709ce`**, `v2/agent/DECISIONS.md` dừng ở **D2-213** (chưa có
D2-214). Trong lúc task chạy, lead đã đẩy `main` lên **`aba8834f`** (vòng 14) có **D2-214** — đã đọc
bản trên `origin/main`: D2-214 mở đúng KT-291 (riscv64 soak 1.000), KT-292 (aarch64), KT-293
(`soak-x86_64.sh` N=600); nội dung **khớp** nhiệm vụ này. Đã đọc thêm D2-190, D2-205, D2-212, D2-213.
Không sửa tệp DECISIONS (ngoài quyền của sub-agent).

**Không** tuyên bố K6 đạt lại: nhiệm vụ chỉ **cung cấp bằng chứng** soak 1.000 trên ảnh hiện tại.

## 1. Vì sao phải soak lại (không tái dùng bằng chứng cũ)

Vòng 13 đổi **mã kernel riscv64** — KT-290 (`Tcb.Configure` invocation từ U-mode, `ktest-ipc` 4 TAP)
— trong `v2/kernel/vifast-core` (`invoke.rs`, `core_tcb.rs`) và `v2/kernel/vifast-kbin-riscv64`
(`init/ipc_client_user.rs`, `src/ktest_ipc.rs`, `src/ktest_user.rs`). Ảnh sản phẩm link **cùng** mã
đó ⇒ `.text` đổi ⇒ bằng chứng soak cũ **không còn áp dụng** (AGENTS §6.3: khóa đầu vào đổi).

| Bằng chứng cũ | Ảnh đo | `build=` / `text=` | Vì sao không tái dùng |
|---|---|---|---|
| KT-284 (`kt284-soak-1000.log`) | sản phẩm, không shell | `e801e262f15d0ac4` / `50702` | mã kernel đổi sau KT-290 |
| KT-78 (`kt78-soak.log`) | sản phẩm, không shell | `ef566cfa583e34bb` / `49874` | mã kernel đổi nhiều vòng |
| KT-138 (`kt138-soak-product/summary.txt`) | phát hành `product-shell,product-fs` | `301720ec292c4f89` / — | ảnh **khác** (có shell) + mã đổi |
| KT-251 (S1) | phát hành `product-shell,product-fs` | `f861bd283066120f` / `131728` | đo **S1**, không phải soak; mã đổi |
| KT-265 | **aarch64** | `68b890daa9e08f6b` / `38072` | kiến trúc khác |

Ảnh đo lần này: `build=8716e60e9aceb940 text=50702` — **khác** KT-284 (`e801e262f15d0ac4`) ⇒ đúng
là ảnh mới, phải đo lại. (Chi tiết trung thực: **độ dài** `.text` trùng KT-284 `50702` B — trùng hợp;
so byte-by-byte thì **586 B khác** giữa hai ảnh, và `inputs` đổi `ab99dd4b4662bbe0` → `70f930a5b594e53e`,
`count` 317 → 323. Không dùng độ dài `.text` làm danh tính.)

## 2. Lệnh + danh tính (khóa đầu vào AGENTS §6.3)

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cd /private/tmp/kt291-s1
SOAK_LOGDIR=/tmp/kt291-soak bash v2/test/boot/soak.sh 1000 3
```

Header nguyên văn (`kt291-soak-1000.log`):

```text
# soak KT-57 — 2026-11-21T07:50:29Z
# qemu 11.1.3 (sàn 9.2, D2-72 Q2); opensbi /opt/homebrew/share/qemu/opensbi-riscv64-generic-fw_dynamic.bin
# cấu hình: -machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot; KHÔNG icount (D2-74 c)
# N=1000 lanes=3 timeout=20s/boot; p=3/1000 (0,3 %) ⇒ n ≥ 3/p = 1000 (D2-24)
# git HEAD=36e709ce64d18653d755b049823faca33b2a030e dirty-kernel-paths=0
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=4.05 loadavg_5m=3.98 loadavg_15m=4.90 ncpu=12 boot_done=0 verdict=OK
# image=product bytes=191296 sha256=a44372b10c6ea5de8fa64b1f66358f6d86975a04e1efa67f037a43c3e38587f1
# BUILD_ID (D2-80) = build=8716e60e9aceb940 text=50702
REV-STATE: root=/private/tmp/kt291-s1 head_rev=36e709ce64d1 inputs=70f930a5b594e53e count=323 dirty=0 scope=[v2/kernel v2/lib v2/Cargo.toml v2/Cargo.lock v2/rust-toolchain.toml v2/tools/repro-build.sh v2/tools/rustc-canon.rs]
REV-GATE: ACCEPT inputs=70f930a5b594e53e current_inputs=70f930a5b594e53e head_rev=36e709ce64d1 count=323 log=/tmp/kt291-soak/kt86-rev-gate.log
```

| Khóa đầu vào | Giá trị |
|---|---|
| commit | `36e709ce64d18653d755b049823faca33b2a030e` (HEAD, cây sạch) |
| build-inputs | `70f930a5b594e53e` (`algo=kt134-v4`, `dirty=0`, `count=323`) |
| toolchain | rustc 1.100.0 (`0ef1bc386` 2026-11-09), cargo 1.100.0 (`3acc79c0c` 2026-11-05) |
| cấu hình | `-machine virt,aia=none,aclint=on -cpu rv64 -m 128M -smp 1 -no-reboot`; **KHÔNG** icount |
| QEMU / OpenSBI | QEMU 11.1.3 / `fw_dynamic` (log in `OpenSBI v1.8.1`) |
| `binary.sha256` | `a44372b10c6ea5de8fa64b1f66358f6d86975a04e1efa67f037a43c3e38587f1` |
| dòng `# build:` của ảnh | `algo=kt134-v4 inputs=70f930a5b594e53e dirty=0` |
| `build=` (D2-80) | `8716e60e9aceb940`, `text=50702` |
| ngân sách (D2-72 Q5) | `data+bss=202232` / trần `262144` (77,1 %) — **không** nới |

## 3. Kết quả — **0/1000**

```text
# soak: bắt đầu N=1000 lanes=3 07:50:41Z
...
# soak: xong wall=248s (4 boot/s); ok=1000 fail=0 tổng=1000/1000
# soak: BUILD_ID=8716e60e9aceb940 sha256=a44372b10c6ea5de8fa64b1f66358f6d86975a04e1efa67f037a43c3e38587f1 git=36e709ce64d18653d755b049823faca33b2a030e
# soak: 1000/1000 lỗi=0 — 0/N chỉ là trần (QD §5.3); không retry (D2-07)
# soak: 0/1000 lỗi (PASS)
# soak.sh: PASS
```

| Phép đo | Kết quả |
|---|---|
| N / luồng / timeout | **1.000** / **3** / **20 s** mỗi boot (không nới) |
| boot đỏ | **0** — `kt291-soak-results.log`: 1000 dòng `ok`, **0** dòng `fail`; chỉ số 1..1000 đủ, không trùng |
| wall | **248 s** (≈ 4 boot/s) — **thời gian KHÔNG HỢP LỆ**, xem §5 |
| guard (`qemu_guard.sh`) | `boot_end=1001 isolated_1=1001 isolated_0=0 rc_0=1001 rc_124=0` (1000 soak + 1 rev-gate) |
| ảnh đổi giữa soak | **không** (sha256 đầu == cuối; `soak.sh` sẽ FAIL nếu đổi) |
| retry | **0** (D2-07) |

## 4. Bất biến mỗi boot (fail-closed) — kiểm trên **cả 1000** log

`soak.sh` kiểm từng boot; tôi kiểm lại **độc lập** trên toàn bộ 1000 tệp `boot-*.log`:

| Bất biến | Số log đạt |
|---|---|
| đúng 1 `# conf image: build=8716e60e9aceb940 text=50702` (khớp ELF, D2-80) | **1000/1000** (chỉ **một** giá trị phân biệt) |
| `# k3 image=product` | **1000/1000** |
| `TAP version 13` + `1..1` + `ok 1 - k3 boot` + `# k3 halt` | **1000/1000** |
| `# build: algo=kt134-v4 inputs=70f930a5b594e53e dirty=0` | **1000/1000** |
| `# smp: boot=0 harts=1 … online=0x1 … timeout=0 … dtb_magic=1` | **1000/1000** |
| `Platform HART Count : 1` (OpenSBI — bằng chứng ngoài kernel) | **1000/1000** |
| `not ok` | **0** |
| `PANIC` (tính cả dòng thụt đầu dòng) | **0** |
| quá 20 s ⇒ kill cả process group, boot đỏ | **0** (`rc_124=0`) |

## 5. Hạ tầng — `MEASUREMENT_INVALID` (ghi trung thực)

```text
# infra before: qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0 loadavg_1m=4.05 loadavg_5m=3.98 loadavg_15m=4.90 ncpu=12 boot_done=0 verdict=OK
# infra after: qemu_total=1 qemu_ppid1=0 qemu_own=0 qemu_foreign=1 loadavg_1m=4.79 loadavg_5m=5.25 loadavg_15m=5.30 ncpu=12 boot_done=1 verdict=KHONG_HOP_LE ly_do=qemu_foreign=1
# MEASUREMENT_INVALID tag=after ly_do=qemu_foreign=1
# infra: invalid=1 isolation_bad=0
```

**Trước khi soak tôi đã kiểm `qemu_foreign=0`** (`ps -ax -o pid,ppid,command | grep qemu` rỗng;
dòng `infra before` xác nhận `qemu_total=0 qemu_ppid1=0 qemu_own=0 qemu_foreign=0`, `verdict=OK`,
`loadavg_1m=4.05 < ncpu=12`). Mẫu `after` báo `KHONG_HOP_LE` vì **`qemu_foreign=1`**: một QEMU của
**sub-agent khác** xuất hiện **trong lúc** soak chạy. Lô vòng 14 có **KT-292** (aarch64 soak,
`/tmp/kt291-s2` nhánh `task/kt292-a64soak4`) và **KT-293** (x86_64 soak, `/tmp/kt291-s3` nhánh
`task/kt293-x64soak`) chạy **song song**; QEMU lạ đã thoát trước khi tôi kịp `ps` nên **không**
xác định được chính xác PID — nhưng chắc chắn **không** phải QEMU của soak này: `qemu_own=0`,
`qemu_ppid1=0`, `isolation_bad=0`, guard `isolated_1=1001 rc_0=1001 rc_124=0`. (Đối chứng: lúc
16:55, `pre-infra` của KT-292 thấy `qemu_total=2 qemu_foreign=2` — chính các QEMU soak của tôi
đang chạy nốt; hai bên nhìn thấy nhau, đúng bản chất "đo song song".)

Hệ quả đúng theo **D2-116/OD-K6-6**: **số thời gian** (`wall=248s`, `4 boot/s`) **không dùng để
kết luận**; **số lỗi 0/1000 vẫn hợp lệ** vì mỗi boot được kiểm **độc lập** (TAP + marker + hash +
smp + firmware) và **không** boot nào chạm timeout (`rc_124=0`). **Không** chạy lại soak để "lấy số
đẹp" (AGENTS §6.3, D2-07).

**Ghi rõ để người đọc không hiểu nhầm**: nhiệm vụ ghi kỳ vọng `invalid=0`; thực đo là
**`invalid=1`** vì một QEMU lạ (của sibling sub-agent) xuất hiện ở mẫu `after`. Đây **không** phải
lỗi của ảnh riscv64 và **không** làm sai số lỗi.

## 6. Không sửa `soak.sh` — không nới ngưỡng

Ảnh sản phẩm hiện tại vẫn in **đúng** các mốc mà `soak.sh` đòi (`TAP version 13`, `1..1`,
`ok 1 - k3 boot`, `# k3 halt`, `# k3 image=product`, `# conf image:`, `# smp:`, `Platform HART
Count : 1`, `OpenSBI v…`) ⇒ **không cần** cập nhật kỳ vọng. Không xoá kiểm nào, không nới ngưỡng
(N=1.000, 3 luồng, 20 s/boot giữ nguyên), không thêm bước vào `v2/bin/v2`.

## 7. Phạm vi bằng chứng — nói rõ cái soak **không** đo

`soak.sh` boot **ảnh sản phẩm không shell** tới `# k3 halt`; nó **không** chạy U-mode/IPC
(những thứ đó nằm ở các ảnh `ktest-*`, đo bằng ktest matrix — KT-290 ghi 27/27). Vì vậy:

- 0/1000 là bằng chứng **ổn định boot** của ảnh sản phẩm có **cùng mã kernel** (gồm
  `Tcb.Configure` invocation của KT-290) — **không** phải bằng chứng stress đường invocation đó
  dưới tải. So `boot-1.log` KT-291 với KT-284: **mọi dòng ổn định giống hệt**; chỉ khác
  `# build:`/`# conf image:` (danh tính ảnh mới) và các dòng **biến thiên theo thời gian**
  (`# hart=0 … time=`, `# timer arm:`, `# timer:`) — đúng kỳ vọng vì đường invocation chỉ chạy
  trong ảnh `ktest-ipc`, không chạy trong boot sản phẩm.
- Ảnh **phát hành** (`product-shell,product-fs`) là **build khác**; soak của nó (KT-138) **không**
  được đo lại trong task này. Nếu lead cần bằng chứng soak cho ảnh phát hành sau vòng 13 thì
  phải chạy riêng `soak.sh --features product-shell,product-fs 1000 3` (ngoài phạm vi KT-291).

## 8. Còn thiếu / rủi ro (ghi rõ, không giấu)

1. **Thời gian soak không hợp lệ** vì QEMU của sibling sub-agent — xem §5. Số lỗi vẫn hợp lệ.
2. **`invalid=1`** (không phải 0 như kỳ vọng nhiệm vụ) — nguyên nhân hạ tầng, không phải ảnh.
3. **Soak phải chạy độc quyền** (AGENTS §6): lần này có sibling `kt292-a64soak4`/`kt293-x64soak`
   chạy song song; không ảnh hưởng 0/1000 nhưng làm số thời gian không dùng được.
4. **Không** tuyên bố K6/K3 đạt lại — chỉ cung cấp bằng chứng.

## 9. Tệp đã đổi

| Tệp | Việc |
|---|---|
| `v2/agent/JOURNAL/kt291-riscv64-soak2.md` | **mới** — tệp này |
| `v2/agent/JOURNAL/kt291-soak-1000.log` | **mới** — toàn bộ console soak (header + 1000 dòng `boot i/1000 ok` + tổng) |
| `v2/agent/JOURNAL/kt291-soak-results.log` | **mới** — `results.txt`: 1000 dòng `ok i`, **0** dòng `fail` |
| `v2/agent/JOURNAL/kt291-guard-summary.log` | **mới** — `boot_end=1001 isolated_1=1001 rc_0=1001 rc_124=0` |
| `v2/agent/JOURNAL/kt291-infra.log` | **mới** — mẫu `qg_infra before/after` (gồm cảnh báo `qemu_foreign`) |
| `v2/agent/JOURNAL/kt291-boot-1.log` | **mới** — log **một** boot đầy đủ (OpenSBI + TAP + `# build:` + `# conf image:` + `# smp:` + `# k3 halt`) |
| `v2/agent/JOURNAL/kt291-rev-gate.log` | **mới** — log boot của cổng danh tính (REV-GATE ACCEPT) |

**Không** sửa mã nguồn, **không** sửa `soak.sh`, **không** sửa `v2/bin/v2`, **không** sửa
`spec/`, **không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`.

## 10. Ràng buộc bằng chứng với commit

Soak chạy trên cây **sạch** tại `HEAD=36e709ce` (`dirty-kernel-paths=0`, `dirty=0`,
`inputs=70f930a5b594e53e`). Commit của task chỉ thêm tệp dưới `v2/agent/JOURNAL/` — **không** nằm
trong build-inputs (bị loại theo `build-rev.sh`) ⇒ dấu vân tay `70f930a5b594e53e` **không đổi** sau
commit; ảnh `build=8716e60e9aceb940 text=50702` áp đúng cho commit này. **Không** chạy lại soak
(cùng fingerprint — AGENTS §6.3 cấm chạy lại soak trên cùng fingerprint chỉ để tăng số lần PASS).
