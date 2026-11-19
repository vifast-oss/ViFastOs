# KT-193 — xác nhận C4 của KT-185 (reviewer độc lập L31)

**Ngày**: 2026-11-19 · **Reviewer**: L31 (độc lập, không phải lead/tác giả) · **Worktree**:
`/private/tmp/night2-L31-c4check` · **Nhánh**: `night2/L31-c4check` (từ `main` @ `8581c218`).
**Phạm vi**: chỉ điều kiện **C4** của KT-185 theo D2-165 §3 — không lặp C1/C2/C5.
**Bản sửa được kiểm**: commit `154eafaf` (đã là tổ tiên của HEAD `8581c218`).
**Ngân sách**: hạn cứng 14:00 JST. Không merge, không push, không sửa `main`, không `gate --fast`,
không QEMU, không sửa mã sản phẩm.

## 0. Kết luận trước

**C4 ĐẠT.** Bốn điều kiện của L26/KT-188 đều đã được sửa đúng; `cargo test` vẫn **47 passed**.
**KT-185 đủ điều kiện chuyển DONE** theo D2-165 §3.

Ghi chú **không chặn C4** (ngoài bốn điều kiện, nên dọn khi cập nhật sổ): còn hai số cũ trong
`kt185-c1c5.md` — dòng 15 ghi "8 trường thiếu" (nay là **19**) và dòng 124 ghi "30/31" (nay là
**28/31**). Xem §3.

## 1. Bốn điều kiện C4

### 1.1 "54 trường" → "58 trường" — ĐẠT

`grep -rn "54 trường"` trong ba tệp: **không còn kết quả**. Số 58 hiện đúng ở cả ba nơi:

| Tệp | Dòng | Nội dung |
|---|---|---|
| `v2/kernel/vifast-plat-aarch64/src/sysreg.rs` | 27 | `DDI 0601 (2024-12) có **58** trường SCTLR_EL1` |
| `v2/agent/JOURNAL/kt167-a64step2.md` | 181 | `DDI 0601 (2024-12) có **58** trường` |
| `v2/agent/JOURNAL/kt185-c1c5.md` | 95 | `DDI 0601 (2024-12) có **58**` |

`grep "54 trường"` không khớp vì markdown bôi đậm (`**58**`); đã kiểm cả `grep "58"` — xem
[`kt193-logs/kt193-grep.log`](kt193-logs/kt193-grep.log). Các lần "54" còn lại đều hợp lệ:
bit 54 của descriptor (`sysreg.rs:20`), `SCTLR_ENASR = 1 << 54` (`sysreg.rs:596`), và câu giải
thích "54 là bản latest 2026-09" (`sysreg.rs:34`, `kt167:185`, `kt185:100`).

### 1.2 Danh sách trường thiếu — đủ 19 tên + bit đúng — ĐẠT

Ba tệp liệt kê **cùng một danh sách 19 trường**, đúng thứ tự và đúng bit:

`nAA` (6), `EnRCTX` (10), `CMOW` (32), `TCF0` (39:38), `TCF` (41:40), `ATA0` (42),
`TWEDEn` (45), `TWEDEL` (49:46), `TMT0` (50), `TMT` (51), `TME0` (52), `TME` (53),
`MSCEn` (33), `EnFPM` (34), `TCSO0` (58), `TCSO` (59), `NMI` (61), `SPINTMASK` (62),
`TIDCP` (63).

Đối chiếu chéo (hai nguồn công khai):

1. **DDI 0601 bản 2024-12** — mirror MRA `fooptrvoid.github.io/arm-mra-2024-12-sysreg`
   (cùng mirror L26 dùng): **58 trường**, có đủ `TME` 53, `TME0` 52, `TMT` 51, `TMT0` 50.
2. **Linux `arch/arm64/tools/sysreg`** tag `v6.6` (nguồn sinh `asm/sysreg.h`):
   `Field 53 TME`, `52 TME0`, `51 TMT`, `50 TMT0`; `nAA` 6, `EnRCTX` 10, `CMOW` 32,
   `TCF0` 39:38, `TCF` 41:40, `ATA0` 42, `TWEDEn` 45, `TWEDEL` 49:46, `MSCEn` 33,
   `NMI` 61, `SPINTMASK` 62, `TIDCP` 63 — khớp.

Kiểm số học: **39 trường đã cài** (`pub const SCTLR_*`, `sysreg.rs:528-604`) + **19 trường thiếu**
= **58**, không trùng, không sót, không sai bit — xem
[`kt193-logs/kt193-fields.log`](kt193-logs/kt193-fields.log).

### 1.3 Câu hedge sai — đã bỏ — ĐẠT

`grep -rn "có thể chưa có\|ARMv9.x"` trong ba tệp: **không còn kết quả** (xem
[`kt193-logs/kt193-stub-hedge.log`](kt193-logs/kt193-stub-hedge.log)). Câu thay thế
"(Số 58 … do reviewer L26/KT-188 đối chiếu DDI 0601 bản 2024-12; bản nháp KT-185 ghi "54" là
**sai** — 54 là bản latest 2026-09 đã bỏ `TME*`.)" khớp log KT-188: bản latest có **54** trường,
không còn `TME*` (`kt188-logs/kt188-arm-fields.log`; mirror latest cũng 54 trường, không `TME*`).

### 1.4 "33 stub" → 31 — ĐẠT

`src/asm.rs` có **31** `pub fn` trong `mod imp` (dòng 330-460; đếm `awk 'NR>=329 && NR<=460'`),
và `kt185-c1c5.md:125` nay ghi "test cả **31** stub". Không còn "33 stub" trong ba tệp.
Xem [`kt193-logs/kt193-stub-hedge.log`](kt193-logs/kt193-stub-hedge.log).

## 2. Test — 47 passed, không hồi quy

`cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64`:

```text
tests/host.rs       24 passed; 0 failed
tests/sysreg.rs     19 passed; 0 failed
tests/tlb_barrier.rs 4 passed; 0 failed
Tổng                47 passed; 0 failed   EXIT=0
```

Log đầy đủ: [`kt193-logs/kt193-test.log`](kt193-logs/kt193-test.log).

## 3. Ghi chú ngoài bốn điều kiện (không chặn C4)

Hai số cũ còn sót trong `v2/agent/JOURNAL/kt185-c1c5.md` — **không** thuộc bốn điều kiện L26,
nhưng nên dọn để journal không tự mâu thuẫn:

| Dòng | Hiện tại | Đúng | Lý do |
|---|---|---|---|
| 15 | "liệt kê đúng tên **8** trường thiếu" | **19** | Bảng tổng kết C4 chưa cập nhật theo bản sửa; thân §3 đã 19 |
| 124 | "**30/31** stub host chưa được kiểm" | **28/31** | 31 stub, `host_stub_is_fail_loud` phủ 3 (`asm::isb` qua `barrier::isb`, `asm::tlbi_vmalle1` qua `tlb::vmalle1`, `asm::read_tcr_el1`) ⇒ 28 chưa phủ |

Hai dòng này không ảnh hưởng bốn điều kiện C4 (danh sách 19 tên ở thân §3 vẫn đúng; tổng stub
vẫn 31), nên **không** chặn việc chuyển KT-185 sang DONE.

## 4. Bằng chứng

- [`kt193-logs/kt193-grep.log`](kt193-logs/kt193-grep.log) — grep "54 trường/đủ 39/33 stub",
  grep mở rộng "8 trường/30/", "58", "31".
- [`kt193-logs/kt193-fields.log`](kt193-logs/kt193-fields.log) — 39 cài + 19 thiếu = 58, đối
  chiếu mirror 2024-12, không lệch bit.
- [`kt193-logs/kt193-stub-hedge.log`](kt193-logs/kt193-stub-hedge.log) — đếm 31 `pub fn`, hedge
  đã bỏ.
- [`kt193-logs/kt193-test.log`](kt193-logs/kt193-test.log) — 47 passed, EXIT=0.

## 5. Lệnh tái lập

```bash
cd /private/tmp/night2-L31-c4check
grep -rn "54 trường\|đủ 39\|33 stub" \
  v2/kernel/vifast-plat-aarch64/src/sysreg.rs \
  v2/agent/JOURNAL/kt167-a64step2.md v2/agent/JOURNAL/kt185-c1c5.md
grep -rn "có thể chưa có\|ARMv9.x" \
  v2/kernel/vifast-plat-aarch64/src/sysreg.rs \
  v2/agent/JOURNAL/kt167-a64step2.md v2/agent/JOURNAL/kt185-c1c5.md
awk 'NR>=329 && NR<=460 && /pub fn/ {c++} END {print c+0}' \
  v2/kernel/vifast-plat-aarch64/src/asm.rs
export PATH="$HOME/.cargo/bin:$PATH"
cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-aarch64
```

## 6. Phán quyết

**C4 ĐẠT** — bốn điều kiện (58 trường, 19 trường thiếu đúng tên+bit, bỏ hedge, 31 stub) đều
được xác nhận trên commit `154eafaf`/HEAD `8581c218`; test 47 passed. **KT-185 đủ điều kiện
chuyển DONE** theo D2-165 §3. Hai số cũ ở §3 là ghi chú dọn dẹp, không chặn.
