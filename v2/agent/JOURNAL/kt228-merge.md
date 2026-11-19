# KT-228 — Giải xung đột merge KT-225 + KT-226 vào nhánh KT-224/KT-227

**Người làm**: L53 (sub-agent của Agent 1) · **Nhánh**: `night3/L53-kt224` · **Gốc**: `80caed7a`
(KT-227) · **Merge**: `main` @ `60ddb1e8` (KT-225 L52 + KT-226 L51).
**Log**: [`kt228-logs/`](kt228-logs/) — selftest, cây thật, 25 ca KT-224, repro `.txt`, `v2 check`.

---

## 0. KẾT QUẢ NGẮN

**ĐẠT.**

| Hạng mục | Kết quả |
|---|---|
| Xung đột | **11 khối** trong `check-wake-depinfo.py`; `check-wake-paths.py` auto-merge (không xung đột) |
| Giữ cả hai bên | KT-225 (4 báo oan + `.txt` + răng B29/B30) **và** KT-227 (fail-open, parse `\ `, `target`, B3b ktest) |
| KT-226 vs KT-227 (trùng nghĩa) | Giữ **một**: `collect_depinfo` của KT-227 (dùng `CWP._is_build_dir` + `skipped` + bỏ lọc đuôi `.rs`); bỏ `_load_dep_files`/`_is_build_dir` wrapper của KT-226; **giữ** 3 ca selftest (21)–(23) của KT-226, đổi số (28)–(30) |
| `--selftest` lớp 1 | **259/259** (KT-225: 246→259; KT-227 đổi 2 kỳ vọng, không đổi số) |
| `--selftest` lớp 2 | **31/31** (KT-227: 27; KT-226: 24; hợp nhất 31) |
| Cây thật (lớp 1 thường + `--strict`, lớp 2) | `errors=0` cả ba |
| 25 ca tấn công KT-224 | **0** đường lọt cả hai lớp, **2** đường lọt riêng lớp 2 (B4b, B17 — lớp 1 bắt), **0** báo oan |
| 4 báo oan KT-225 | **XANH** (selftest FP1/FP1b/FP2/FP3/FP4) |
| Repro `.txt` | **ĐỎ cả hai lớp** (lớp 1 `E_STRICT_MACRO_INCLUDE`, lớp 2 `E_DEPINFO_WAKE`) |
| `v2 check` | **PASS** |

---

## 1. XUNG ĐỘT VÀ CÁCH GIẢI (11 khối, `check-wake-depinfo.py`)

| # | Vùng | HEAD (KT-227) | main (KT-226) | Chọn |
|---|---|---|---|---|
| 1 | đầu `collect_depinfo` | `def collect_depinfo(` | `_is_build_dir` wrapper + `_load_dep_files` | **HEAD** — `collect_depinfo` (đã dùng `CWP._is_build_dir`) |
| 2 | chữ ký + docstring | `use_all`, `artifacts` | `artifacts=None`, `use_all=True` | **HEAD** |
| 3 | lọc artifact | `not use_all and not (targets & artifacts)` | thêm `artifacts is not None` | **HEAD** (artifacts luôn là set) |
| 4 | thân vòng lặp | `skipped` + `CWP._is_build_dir` + không lọc đuôi | lọc `.rs` + `_is_build_dir` wrapper | **HEAD** |
| 5 | `depinfo_empty_errors` | có (P0-1) | không có | **HEAD** |
| 6 | gọi trong `run_unit` | `collect_depinfo(...)` | `_load_dep_files(...)` | **HEAD** |
| 7 | miễn producer | bỏ miễn cả tệp + đếm `producer_files` | bỏ miễn cả tệp | **GỘP** — bỏ miễn cả tệp (cả hai), giữ đếm `producer_files` |
| 8 | selftest (5)/(6) | `producer_files` | `exempt_producer` + ca (6b) | **GỘP** — (5)/(6) của HEAD + ca (6b) của main |
| 9 | selftest (21)–(27) | 7 ca KT-227 | 3 ca KT-226 | **GỘP** — 7 ca KT-227 + 3 ca KT-226 đổi số (28)–(30), dùng `CWP._is_build_dir`/`collect_depinfo` |
| 10 | `--no-build` | `collect_depinfo(...)` | `_load_dep_files` + `if deps_dir.is_dir()` | **HEAD** |
| 11 | dòng in `quét:` | `producer_files` | `exempt_test` + ghi chú KT-226 | **HEAD** |

**Vì sao chọn KT-227 làm bản chính cho bộ lọc `target` + producer**: KT-227 có **đủ** ba thứ
mà KT-226 không có — (a) `res.skipped` + `E_DEPINFO_OUTSIDE`/`W_DEPINFO_SKIPPED` (không im lặng),
(b) **bỏ lọc đuôi `.rs`** (đóng B9/B12), (c) `depinfo_empty_errors` (P0-1). KT-226 chỉ có bộ lọc
`target` + bỏ miễn cả tệp — **trùng nghĩa** với KT-227, nên giữ một. Ca selftest của KT-226
**không bị xoá**: (6b) giữ nguyên, (21)–(23) đổi số (28)–(30) và dùng hàm chung.

**`check-wake-paths.py`**: auto-merge sạch — KT-225 (báo oan + `.txt` + răng) và KT-227
(`cfg_is_test_tokens(ktest_is_test=False)`) cùng có mặt; `git diff main` chỉ còn đúng thay đổi
KT-227.

---

## 2. KIỂM SAU MERGE

### 2.1 Selftest

```
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 259/259 ok
$ python3 v2/tools/check-wake-depinfo.py --selftest
# selftest: 31/31 ok
```

Log: `kt228-logs/selftest-l1.log`, `selftest-l2.log`.

### 2.2 Cây thật

```
$ python3 v2/tools/check-wake-paths.py --root "$PWD"
# check-wake-paths: mode=v2 strict=0 errors=0 warnings=0
$ python3 v2/tools/check-wake-paths.py --root "$PWD" --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
$ python3 v2/tools/check-wake-depinfo.py --root "$PWD"
# check-wake-depinfo: errors=0 warnings=0
```

Log: `kt228-logs/l1-thuong.log`, `l1-strict.log`, `l2-chay-that.log`.

### 2.3 Bốn báo oan KT-225 — XANH

Selftest lớp 1 (259/259) chứa đúng 5 ca `KT-225 FP1/FP1b/FP2/FP3/FP4`; log
`kt228-logs/kt225-fp.log` trích các dòng `ok`. Đây là các dạng:
`stringify!(include!)`, `stringify!(include!())`, `include != 0`, `self.include != 0`,
`include!("…",)` — tất cả **XANH** sau merge.

### 2.4 Repro `.txt` — ĐỎ cả hai lớp

Dựng lại repro của KT-225 §2: `macro_rules!` trong `kt228_macro_def.txt` (không phải `.rs`)
được `include!` vào `kt228_txt.rs`, thân macro chứa `include!("../../../lib/kt228_evil_expr.rs")`,
tệp đích chứa `m.wake(t)`:

```
cargo check -p vifast-core RC=0
L1 thường:   RC=1  E_STRICT_MACRO_INCLUDE  kt228_macro_def.txt:3
L1 --strict: RC=1  E_STRICT_MACRO_INCLUDE  kt228_macro_def.txt:3
L2 dep-info: RC=1  E_DEPINFO_WAKE         v2/lib/kt228_evil_expr.rs:1
```

Log: `kt228-logs/repro-txt.log`.

### 2.5 25 ca tấn công KT-224

Harness: [`kt228-logs/harness-25-ca.py`](kt228-logs/harness-25-ca.py) (bản KT-227, chạy trên
công cụ **đã merge**); bảng máy đọc: `kt228-logs/bang-tan-cong.json`.

| Nhóm | Số ca | Kết quả |
|---|---|---|
| Cả hai lớp ĐỎ | 18 | B1, B2, B3a, B4a, B5, B6, B7, B8, B9, B10, B11, B18, B19 + … |
| Lớp 2 ĐỎ / lớp 1 XANH | 5 | B12, B13, B14, B15, B16 (đều trong `v2/lib` — lớp 1 không có luật token ở đó) |
| Lớp 1 ĐỎ / lớp 2 XANH | 2 | B3b (ktest, feature tắt), B4b (`cfg(any())`, luôn tắt) |
| Lớp 2 XANH có cảnh báo / lớp 1 ĐỎ | 1 | B17 (OUT_DIR — `W_DEPINFO_SKIPPED`) |
| Đối chứng dương XANH | 4 | C1a–C1d |

**Đường lọt cả hai lớp: 0. Báo oan: 0.**

---

## 3. TỆP ĐÃ CHẠM

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-depinfo.py` | giải 11 khối xung đột: giữ KT-227 (`collect_depinfo`, `depinfo_empty_errors`, `skipped`, parse `\ `) + KT-226 (ca selftest (6b), (28)–(30)); thêm lại helper `is_producer_file` (chỉ đếm) |
| `v2/tools/check-wake-paths.py` | auto-merge: KT-225 + KT-227 cùng có mặt |
| `v2/agent/JOURNAL/kt228-merge.md` | **mới** (tệp này) |
| `v2/agent/JOURNAL/kt228-logs/**` | **mới** |

**Không** xoá ca selftest của bên nào; **không** nới ngưỡng; **không** thêm allowlist để lấy
xanh; **không** push; **không** `gate --fast`.

## Phụ lục — Cách tái lập

```bash
cd /private/tmp/night3-L53-kt224
export PATH="$HOME/.cargo/bin:$PATH"
python3 v2/tools/check-wake-paths.py --selftest          # 259/259
python3 v2/tools/check-wake-depinfo.py --selftest        # 31/31
python3 v2/agent/JOURNAL/kt228-logs/harness-25-ca.py     # 25 ca (≈15 phút)
./v2/bin/v2 check                                        # PASS
```
