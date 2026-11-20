# KT-241 — bộ tấn công của reviewer E (ma trận 27 ô)

**Vai**: reviewer độc lập E (khác tác giả A/B/C/D). **Worktree**: `/private/tmp/kt241-E-audit`,
nhánh `kt241/E-audit`, base `c5a83552`. **Phạm vi**: chỉ tạo thư mục này + `kt241-review.md`;
**không** sửa mã sản phẩm/checker/`v2/bin/v2`/`spec/**`.

**Trạng thái lúc soạn**: API `NonReadyTarget` + `state_write` **chưa có** trong cây (A/B/C/D đang
chạy song song). Vì vậy:
- `./run.sh --dry-run` **chạy được** (in kế hoạch, không chèn tệp) — log `logs/dry-run.log`.
- `./run.sh --preflight` báo **FAIL 5 mục** đúng như dự kiến (API chưa hợp nhất) — log `logs/preflight.log`.
- `./mock/check.sh` tự kiểm **mã lỗi cargo** của 18 ô `core`/`core2` trên một **mock API ngoài cây**
  — `PASS=18 FAIL=0`, log `logs/mock-validation.log`. Đây **không** phải bằng chứng nghiệm thu.
- **Chưa chạy tấn công thật vào cây** (đúng yêu cầu: API chưa có ⇒ lỗi biên dịch vô nghĩa).

## 1. Bố cục

```
kt241-attack/
├── README.md              # tệp này
├── manifest.tsv           # 27 ô: id/group/kind/kỳ vọng/đối chứng dương/mô tả
├── run.sh                 # script chạy: --dry-run | --preflight | --list | --all | --group | --cell
├── cells/                 # 27 tệp tấn công + 27 đối chứng dương + tệp phụ, đuôi .rs.txt
├── mock/check.sh          # tự kiểm mã lỗi cargo trên mock API (ngoài cây)
├── logs/                  # dry-run, preflight, list, mock-validation (do E chạy)
└── results/               # current.tsv + log từng ô (do lead sinh khi chạy thật)
```

Tệp tấn công để đuôi **`.rs.txt`** để **không** bị cargo biên dịch và **không** bị checker quét
(`v2/agent/**` ngoài phạm vi; `.rs.txt` không phải `.rs`). `run.sh` chép tạm thành `.rs` trong
lúc đo rồi **khôi phục bằng trap** (`EXIT/INT/TERM`), không dùng `git checkout/stash/reset`.

## 2. Cách chạy

```bash
cd /private/tmp/kt241-E-audit          # hoặc worktree tích hợp cuối
A=v2/agent/JOURNAL/kt241-attack

$A/run.sh --dry-run                    # in kế hoạch (chạy được ngay)
$A/run.sh --preflight                  # kiểm API/checker/toolchain (exit 2 nếu API chưa có)
$A/run.sh --list                       # bảng 27 ô
$A/run.sh --all --with-depinfo         # CHẠY THẬT đầy đủ (sau hợp nhất; ~40–60 phút)
$A/run.sh --group A --with-depinfo     # một nhóm
$A/run.sh --cell E7                    # một ô (E7 = đột biến mỏ neo)
$A/run.sh --cell C1 --keep             # giữ tệp ô cuối để debug (trap không khôi phục)
```

- `--all` chạy **baseline** trước: nếu cây nền ĐỎ (l1/l1s/l2) thì **dừng** (exit 4) — kết quả
  ma trận sẽ vô nghĩa trên nền đỏ.
- `--with-depinfo` bật **lớp 2** (`check-wake-depinfo.py`). Không có cờ này, cột L2 = `skip`.
- Mỗi ô: backup tệp sẽ sửa → chèn → `cargo check` → lớp 1 thường → lớp 1 `--strict` → (lớp 2)
  → khôi phục → chạy **đối chứng dương** (cargo OK + l1/l1s XANH) → khôi phục.
- Kết quả: `results/current.tsv` (bảng) + `logs/<ID>/{cargo,l1,l1s,l2,positive-*}.log`.

## 3. Manifest

Cột: `id | group | kind | expect_cargo | expect_codes_cargo | expect_l1 | expect_l1s | expect_l2 |
expect_codes_checker | positive | positive_kind | desc`.

- `expect_cargo`: `ok` | `fail` | `skip`.
- `expect_codes_cargo`: regex mã lỗi cargo (`E0308`, `E0624`, `E0631`, `unsafe`).
- `expect_l1/l1s/l2`: `red` | `green` | `any` | `skip`.
- `expect_codes_checker`: regex mã checker (`E_STRICT_*`, `E_DEPINFO_*`, `E_ANCHOR_*`).
- `positive`: có tệp đối chứng dương `cells/<ID>.positive.rs.txt` hay không.

## 4. 27 ô

| Nhóm | Ô | Dạng | Kỳ vọng chính |
|---|---|---|---|
| A (4) | A1 | alias `use TcbState as S; S::Ready` | cargo **E0308**; checker any |
| | A2 | UFCS `Machine::transition_tcb` / `<Machine>::` | cargo **E0308** |
| | A3 | fn-pointer / generic `F: Fn` / trait object | cargo **E0308/E0631** |
| | A4 | `<S>::Ready` + path-qualified `TcbState::Ready` | cargo **E0308** |
| B (6) | B1 | helper cùng tệp nhận `TcbState` | cargo **E0308** |
| | B2 | helper tệp khác nhận `TcbState` | cargo **E0308** |
| | B3 | hàm trả về `TcbState::Ready` | cargo **E0308** |
| | B4 | `const`/`static` kiểu `TcbState` | cargo **E0308** |
| | B5 | closure nhận `TcbState` | cargo **E0308** |
| | B6 | **đối chứng dương**: `block_current` + biến thể block | cargo OK + l1/l1s/l2 **XANH** |
| C (5) | C1 | `macro_rules!` che `matches` → `m.wake(t)` | cargo OK; l1 **ĐỎ** `E_STRICT_TEST_ONLY_WAKE`; l2 **ĐỎ** `E_DEPINFO_WAKE` |
| | C2 | `include!` literal tệp trong `src/` chứa `wake` | cargo OK; l1/l2 **ĐỎ** |
| | C3 | `#[path]` trong inline module tệp non-mod-rs ra ngoài `src/` | cargo OK; l1 **ĐỎ** `E_STRICT_WAKE_INCLUDE`; l2 **ĐỎ** |
| | C4 | `build.rs` + `$OUT_DIR` + `include!` không literal | cargo OK; l1 **ĐỎ** `E_STRICT_WAKE_INCLUDE`; l2 **ĐỎ** |
| | C5 | `#[cfg(any())] mod` che module chứa `wake` | cargo OK; l1 **ĐỎ**; l2 **XANH** (giới hạn: lớp 2 chỉ thấy tệp rustc nạp) |
| D (5) | D1 | crate thành viên ngoài ba gốc + `$OUT_DIR` (WB-18) | cargo OK; l1 **XANH** (ngoài ba gốc); l2 **ĐỎ** `E_DEPINFO_WAKE` |
| | D2 | tên tệp có dấu cách nạp bằng `#[path]` | cargo OK; l1/l2 **ĐỎ** |
| | D3 | thư mục `src/target/` (WB-12) | cargo OK; l1/l2 **ĐỎ** |
| | D4 | dependency mới từ crate sẵn có ngoài ba gốc | cargo OK; l1 **XANH**; l2 **ĐỎ** (KT-238 nâng lên bảo vệ) |
| | D5 | `#[path]` mức mục ra ngoài `src/` | cargo OK; l1 **ĐỎ**; l2 **ĐỎ** |
| E (7) | E1 | `impl From<TcbState> for NonReadyTarget` | cargo OK; l1 **PHẢI ĐỎ** (luật mới của C); l2 any |
| | E2 | `impl TryFrom<TcbState> for NonReadyTarget` | cargo OK; l1 **PHẢI ĐỎ**; l2 any |
| | E3 | `into_state` từ module ngoài `state_write` | cargo **E0624**; checker any |
| | E4 | wrapper nhận `TcbState` gọi `transition_tcb` | cargo **E0308** |
| | E5 | `write_state` từ module anh em | cargo **E0624**; l1 **PHẢI ĐỎ** (bảo vệ primitive) |
| | E6 | `unsafe transmute` (core `forbid(unsafe_code)`) | cargo **unsafe** |
| | E7 | đột biến: đổi chữ ký `transition_tcb` về `TcbState` | checker **PHẢI ĐỎ** (mỏ neo) |

## 5. Tiêu chí đánh giá sau thu hẹp API

- **Ô compiler chặn được** (A1–A4, B1–B5, E3–E6): `cargo` **phải** từ chối **đúng mã**
  (`E0308`/`E0631`/`E0624`/`unsafe`). Checker `any` — ĐỎ là phòng thủ nhiều lớp, XANH chấp nhận
  vì ranh giới kiểu đã chặn; nếu ĐỎ phải đúng mã.
- **Ô compiler KHÔNG chặn được** (C1–C5, D1–D5, E1, E2, E5, E7): checker **phải** ĐỎ **đúng mã**.
  Đây là các đường mà ranh giới kiểu không cứu được (macro/cfg/nguồn sinh/phạm vi/`From`/primitive).
- **Đối chứng dương** phải XANH: chứng minh ĐỎ không phải do tệp bị bỏ qua hay báo oan.

## 6. Giới hạn / điều cần biết

1. **Mã checker của E1/E2/E5/E7 là tạm định** (`E_STRICT_TRANSITION|E_STRICT_NONREADY|...`).
   Sau khi C chốt mã thật, cập nhật cột `expect_codes_checker` trong `manifest.tsv` — nếu không,
   các ô này sẽ FAIL vì **sai mã**, không phải vì lỗ hổng.
2. **C5 lớp 2 XANH có chủ đích**: `#[cfg(any())]` không được biên dịch ⇒ tệp không vào `.d`;
   lớp 1 quét mọi nhánh cfg nên vẫn ĐỎ. Đây là giới hạn đã công bố của lớp 2 (D2-173 §3).
3. **C4 đối chứng dương** dùng `include!` **literal trong `src/`** (kind `core`), không dùng
   `$OUT_DIR` lành — vì lớp 1 **cố ý** từ chối mọi `include!` không literal (báo oan đã biết,
   KT-233 §5). Đừng "sửa" đối chứng dương thành `$OUT_DIR` lành rồi coi ĐỎ là hồi quy.
4. **D1/D4 lớp 1 XANH có chủ đích**: `v2/extra/**` và `v2/tools/**` ngoài ba gốc quét của lớp 1;
   lớp 2 (KT-238) là lớp bắt. Nếu hợp nhất đưa lớp 1 sang phạm vi `cargo metadata` thì đổi
   `expect_l1` thành `red` và cập nhật manifest.
5. **E7 là đột biến mã sản phẩm**, không phải tệp tấn công: `run.sh` sửa chữ ký
   `transition_tcb(... to: NonReadyTarget)` → `TcbState` trong `state_write.rs`, chạy checker,
   rồi khôi phục. Nếu không tìm thấy chữ ký, ô báo `ERROR` (không phải FAIL).
6. **Không chạy song song** với `gate --fast`/`nightly` (D2-192: đo nặng độc quyền).
