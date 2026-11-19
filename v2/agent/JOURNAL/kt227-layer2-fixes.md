# KT-227 — Vá lớp 2 sau review KT-224 (P0-1..P0-4, P1-5, P1-6)

**Người làm**: L53 (sub-agent của Agent 1) · **Nhánh**: `night3/L53-kt224` · **Gốc**: `7a7787fc`
(KT-224 review) · **Commit**: xem `git log -1`.
**Nguồn**: review độc lập KT-224 — [`kt224-review214.md`](kt224-review214.md).
**Log**: [`kt227-logs/`](kt227-logs/) — selftest, `v2 check`, 25 ca tấn công chạy lại, log `.d`.

---

## 0. KẾT QUẢ NGẮN

**ĐẠT** — cả bốn P0 đã vá và **kiểm lại bằng đúng 25 ca tấn công của KT-224**: số đường lọt
**cả hai lớp** từ **5 → 0**; số đường lọt **riêng lớp 2** từ **8 → 2** (B4b, B17 — cả hai đều bị
lớp 1 bắt); **0 báo oan** (4/4 đối chứng dương XANH).

| Hạng mục | Trước KT-227 | Sau KT-227 |
|---|---|---|
| Đường lọt **cả hai lớp** | **5** (B3b, B12, B13, B14, B15) | **0** |
| Đường lọt **riêng lớp 2** (lớp 1 bắt) | **8** (B4b, B7–B11, B17, B18) | **2** (B4b, B17) |
| Đường lọt **riêng lớp 1** (lớp 2 bắt) | 1 (B16) | 5 (B12–B16) |
| Báo oan | 0 | **0** |
| `--selftest` lớp 2 | 20/20 | **27/27** |
| `--selftest` lớp 1 | 246/246 | **246/246** |
| `--strict` trên cây thật | `errors=0` | **`errors=0`** |
| `v2 check` | PASS | **PASS** |

---

## 1. P0-1 — Fail-open `.d` rỗng/thiếu ⇒ ĐỎ

**Lỗi**: `.d` rỗng hoặc không tồn tại ⇒ `errors=0`, RC=0, **0 tệp quét**. Vi phạm trực tiếp yêu
cầu "phải fail-closed, không im lặng xanh".

**Vá** (`check-wake-depinfo.py`):
- `depinfo_empty_errors(results, loaded)` — hàm mới: `loaded` rỗng ⇒ lỗi; mỗi đơn vị có
  `depinfo_used == 0` hoặc `loaded` rỗng ⇒ lỗi.
- `main` gọi hàm này sau `scan` ⇒ `E_DEPINFO_EMPTY` (ĐỎ).
- `--no-build` dùng chung `collect_depinfo` nên cũng đi qua cùng kiểm.

**Kiểm**: selftest (23) `depinfo_empty_errors` ĐỎ khi 0 tệp; (24) rỗng khi có tệp; (25)
**end-to-end** `main(["--root", fake, "--no-build"])` ⇒ RC=1 + `E_DEPINFO_EMPTY`.

```
$ python3 v2/tools/check-wake-depinfo.py --root <cây không .d> --no-build
E E_DEPINFO_EMPTY   không đọc được tệp `.rs` nào từ `.d` — fail-closed (KT-227 P0-1)…
E E_DEPINFO_EMPTY   đơn vị `host`: không có tệp `.d` nào được dùng — fail-closed…
RC=1
```

Log: `kt227-logs/p01-fail-open.log`.

---

## 2. P0-2 — Parse `\ ` (escape dấu cách)

**Lỗi**: `parse_depinfo` dùng `rhs.split()` **trước** khi giải escape ⇒ `src/a\ b.rs` thành
`['src/a\\', 'b.rs']`; tên tệp có dấu cách không bao giờ được quét. Selftest cũ chỉ kiểm
`_unescape_dep` trực tiếp nên **pass trong khi parser sai**.

**Vá**:
- `_split_dep_tokens(s)` — tách token **tôn trọng** `\ `, `\\`, `\#` (không `split()` trước).
- `_join_continuations(text)` — nối dòng theo **số lẻ** dấu `\` cuối dòng (bản cũ
  `replace("\\\n", " ")` nối nhầm tên tệp kết thúc bằng `\\`).
- `parse_depinfo` bỏ dòng bắt đầu `#` (make comment, vd `# env-dep:OUT_DIR=…`) — bản cũ coi
  `OUT_DIR=…` là phụ thuộc.

**Kiểm**: selftest (21) `parse_depinfo` với `a\ b.rs` ⇒ `["src/a b.rs", "src/c.rs"]`; (22) nối
dòng + `\\` literal + dòng `#`. Log: `kt227-logs/p02-parse.log`.

---

## 3. P0-3 — Bộ lọc `target` + miễn theo tệp + báo tệp bị bỏ

**Ba lỗi**:
1. `"target" in relp.parts` — hồi quy đường lọt #12 (`src/target/evil.rs` bị bỏ).
2. `p.suffix != ".rs"` — `#[path = "evil.txt"] mod m;` là mã Rust thật, bị bỏ.
3. `is_producer_file` miễn **cả tệp** `core_sched.rs`/`kobj.rs` ⇒ `m.wake(t)` thêm vào đó lọt.
4. Tệp ngoài gốc repo / symlink ra ngoài bị bỏ **im lặng**.

**Vá**:
- `collect_depinfo(...)` — hàm mới dùng chung cho `run_unit` và `--no-build`:
  - bộ lọc `target` dùng **chung** `CWP._is_build_dir` (chỉ bỏ khi `target` đứng **trước** `src`);
  - **bỏ** lọc đuôi `.rs` — quét mọi tệp rustc nạp;
  - mọi tệp bị bỏ ghi vào `res.skipped` kèm lý do.
- `main`: `skipped` lý do **"ngoài gốc repo"** ⇒ `E_DEPINFO_OUTSIDE` (ĐỎ, fail-closed); lý do
  khác (thư mục build / không phải tệp) ⇒ `W_DEPINFO_SKIPPED` (CẢNH BÁO, không im lặng).
- `scan`: **bỏ** `continue` của `is_producer_file`; chỉ đếm `producer_files` để báo. Việc miễn
  **định nghĩa** `fn wake` do `CWP._check_wake_text` làm — đúng mức.

**Kiểm**: selftest (26) `.txt` + `src/target/` được nạp, tệp ngoài gốc bị `skipped`; (27) tệp có
`fn wake` **vẫn** bị bắt nếu có `m.wake(t)`. Log: `kt227-logs/p03-target-producer.log`.

---

## 4. P0-4 — B3b: `#[cfg(feature = "ktest")] mod` không còn test-only

**Lỗi**: `test_only_modules` coi module dưới `#[cfg(feature = "ktest")]` là test-only ⇒ lớp 1
miễn; lớp 2 không nạp (feature tắt) ⇒ **cả hai lớp mù** (đường lọt B3b).

**Vá** (`check-wake-paths.py`):
- `cfg_is_test_tokens(nodes, ktest_is_test=False)` — `feature = "ktest"` **không** còn là test
  theo mặc định. Không có bằng chứng `ktest` **không bao giờ** bật trong build sản phẩm (nó là
  feature của `vifast-kbin-riscv64`/`vifast-plat-riscv64`) ⇒ **fail-closed**, coi là sản phẩm.
- Hai ca selftest đổi kỳ vọng: (4v) `B8b` và (4ai) `KT-172 A2` — trước XANH, nay **ĐỎ**.

**Báo oan trên cây thật: 0.** Module duy nhất dưới `#[cfg(feature = "ktest")]` là
`v2/kernel/vifast-plat-riscv64/src/probe.rs` (139 dòng, **không** có token `wake`); toàn bộ tệp
có `cfg(feature = "ktest")` trong `v2/kernel`+`v2/sim` **không** tệp nào có `wake`. Lớp 1 thường
+ `--strict` trên cây thật: `errors=0` (`kt227-logs/l1-thuong.log`, `l1-strict.log`).

**Kiểm lại B3b**: lớp 1 nay **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` (`kt227-logs/b-b3b-layer1.log`);
lớp 2 vẫn XANH (feature tắt — đúng, lớp 1 đã bắt).

---

## 5. P1-5 — Threat model `v2/lib`: **không nguy hiểm hôm nay**, nợ có tên

**Câu hỏi**: tệp trong `v2/lib` có với tới `Machine` **thật** (có `wake`) không?

**Trả lời: KHÔNG** — ba bằng chứng:
1. Không crate `v2/lib/*` nào phụ thuộc `vifast-core`/`vifast-sim` ở `[dependencies]`.
   `vifast-model` và `vifast-traceformat` chỉ có `vifast-sim` ở **`[dev-dependencies]`** (test,
   không vào ảnh sản phẩm).
2. `vifast-model::Machine` (crate `v2/lib` duy nhất định nghĩa `Machine`) **không có** phương
   thức `wake` — `grep "pub fn .*wake" v2/lib/vifast-model/src/machine.rs` rỗng.
3. `vifast-core::Machine::wake` và `vifast-sim::Machine::wake` là `pub` nhưng crate chứa chúng
   **không** nằm trong đồ thị phụ thuộc sản phẩm của `v2/lib`.

⇒ B12–B15 (`.txt`/symlink/`src/target/`/dấu cách trong `v2/lib`) **không phải** đường gọi
`wake()` thật hôm nay; chúng là lỗ **phủ** của luật token (luật không phân biệt kiểu). Sau
P0-2/P0-3, **lớp 2 đã bắt cả bốn** (B12–B15 nay lớp 2 ĐỎ) — nên lỗ đã đóng ở mức công cụ.

**Nợ có tên** (`DEBT-KT227-1`): lớp 1 **không** áp luật token `wake` trên `v2/lib` (hợp đồng
`CHECKER-WAKE-CONTRACT.md` §1 nói rõ). Nếu mai kia (a) `vifast-model::Machine` được thêm `wake`,
hoặc (b) một crate `v2/lib` nhận `vifast-core`/`vifast-sim` vào `[dependencies]`, thì lỗ thành
thật. Điều kiện đóng nợ: khi (a) hoặc (b) xảy ra, mở rộng `RE_TEST_ONLY_WAKE_SCOPE` sang
`v2/lib/**` (hoặc thêm luật riêng). Hôm nay **không** mở rộng vì sẽ báo oan hàng loạt (mã
`v2/lib` không có `Machine`).

---

## 6. P1-6 — Đính chính luận cứ đã cũ

Đã sửa (kèm ghi chú "ĐÍNH CHÍNH KT-227", giữ nguyên số đo lịch sử):

| Tệp | Sửa |
|---|---|
| `JOURNAL/kt214-depinfo-prototype.md` | §0 bảng, §3 tổng, §6 #6, §7.1 — "tĩnh 5/11" là số đo ở `dc6e01ae`; trên HEAD `83275d94` tĩnh bắt **11/11**; #6 sai một phần (lớp 1 cũng mù với `ktest mod`) |
| `JOURNAL/kt217-depinfo-layer2.md` | §2, §3, §7 — cùng đính chính; luận cứ "lấp 6/11" không còn đúng |
| `v2/agent/QUEUE.md` | dòng KT-214 và KT-217 — thêm kết quả review KT-224 + đính chính |

**Bằng chứng tái lập**: `kt224-logs/p02-p07-tai-lap-head.log` — p02–p06 ĐỎ
`E_STRICT_WAKE_INCLUDE`, p07 ĐỎ `E_STRICT_TEST_ONLY_WAKE` trên HEAD.

---

## 7. KIỂM LẠI 25 CA TẤN CÔNG CỦA KT-224

Harness: [`kt227-logs/harness-kiem-lai.py`](kt227-logs/harness-kiem-lai.py) (bản sao harness
KT-224, chạy trên công cụ **đã vá**); bảng máy đọc: `kt227-logs/bang-tan-cong.json`.

| Ca | Cơ chế | Lớp 2 trước → sau | Lớp 1 trước → sau |
|---|---|---|---|
| B1 | `macro_rules!` thân chứa `wake` | ĐỎ → ĐỎ | ĐỎ → ĐỎ |
| B2 | `macro_rules!` nhận `wake` làm đối số | ĐỎ → ĐỎ | ĐỎ → ĐỎ |
| B3a | `#[cfg(feature = "ktest")]` khối trong `lib.rs` | ĐỎ → ĐỎ | ĐỎ → ĐỎ |
| **B3b** | `#[cfg(feature = "ktest")] mod` tệp riêng | XANH → XANH | **XANH → ĐỎ** ✅ |
| B4a | `#[cfg(any())]` khối trong `lib.rs` | ĐỎ → ĐỎ | ĐỎ → ĐỎ |
| B4b | `#[cfg(any())] mod` tệp riêng | XANH → XANH | ĐỎ → ĐỎ |
| B5 | `#[cfg(target_os = "none")] mod` | ĐỎ → ĐỎ | ĐỎ → ĐỎ |
| B6 | `include!` tệp không tồn tại | ĐỎ → ĐỎ | ĐỎ → ĐỎ |
| **B7** | `#[path]` ra `/tmp` | **XANH → ĐỎ** ✅ | ĐỎ → ĐỎ |
| **B8** | symlink ra `/tmp` | **XANH → ĐỎ** ✅ | ĐỎ → ĐỎ |
| **B9** | `#[path = "*.txt"]` | **XANH → ĐỎ** ✅ | ĐỎ → ĐỎ |
| **B10** | `#[path]` tên có dấu cách | **XANH → ĐỎ** ✅ | ĐỎ → ĐỎ |
| **B11** | `#[path]` vào `src/target/` | **XANH → ĐỎ** ✅ | ĐỎ → ĐỎ |
| **B12** | `.txt` trong `v2/lib` | **XANH → ĐỎ** ✅ | XANH → XANH |
| **B13** | symlink trong `v2/lib` | **XANH → ĐỎ** ✅ | XANH → XANH |
| **B14** | `src/target/` trong `v2/lib` | **XANH → ĐỎ** ✅ | XANH → XANH |
| **B15** | dấu cách trong `v2/lib` | **XANH → ĐỎ** ✅ | XANH → XANH |
| B16 | Unicode trong `v2/lib` | ĐỎ → ĐỎ | XANH → XANH |
| B17 | OUT_DIR/build.rs | XANH → XANH (**có `W_DEPINFO_SKIPPED`**) | ĐỎ → ĐỎ |
| **B18** | `m.wake(t)` trong `core_sched.rs` | **XANH → ĐỎ** ✅ | ĐỎ → ĐỎ |
| B19 | `#[cfg(test)]` nhánh `match` (#13) | ĐỎ → ĐỎ | ĐỎ → ĐỎ |
| C1a–C1d | đối chứng dương | XANH → XANH | XANH → XANH |

**Còn lại 2 đường lọt riêng lớp 2** (đều bị lớp 1 bắt — không phải lỗ cổng):
- **B4b** `#[cfg(any())] mod` tệp riêng: lớp 2 không nạp (cfg luôn tắt) — **giới hạn cứng** đã
  ghi trong journal KT-214 (#6); lớp 1 quét mọi `.rs` nên bắt.
- **B17** OUT_DIR/build.rs: tệp sinh nằm dưới `target/` ⇒ lớp 2 **báo** `W_DEPINFO_SKIPPED`
  (không còn im lặng) nhưng không quét; lớp 1 bắt `include!` không literal.

---

## 8. CỔNG

```
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 246/246 ok
$ python3 v2/tools/check-wake-depinfo.py --selftest
# selftest: 27/27 ok
$ python3 v2/tools/check-wake-paths.py --root "$PWD" --strict
# check-wake-paths: mode=v2 strict=1 errors=0 warnings=0
$ python3 v2/tools/check-wake-depinfo.py --root "$PWD"
# check-wake-depinfo: errors=0 warnings=0
$ ./v2/bin/v2 check
v2 check: PASS
```

Log: `kt227-logs/{l1-thuong,l1-strict,l2-chay-that,v2check}.log`.

---

## 9. TỆP ĐÃ CHẠM

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-depinfo.py` | P0-1 (`depinfo_empty_errors`), P0-2 (`_join_continuations`/`_split_dep_tokens`), P0-3 (`collect_depinfo`, `skipped`, bỏ miễn cả tệp), 7 ca selftest mới (20→27) |
| `v2/tools/check-wake-paths.py` | P0-4 (`cfg_is_test_tokens(ktest_is_test=False)`), 2 ca selftest đổi kỳ vọng (246 giữ nguyên) |
| `v2/agent/JOURNAL/kt214-depinfo-prototype.md` | P1-6 đính chính |
| `v2/agent/JOURNAL/kt217-depinfo-layer2.md` | P1-6 đính chính |
| `v2/agent/QUEUE.md` | P1-6 đính chính dòng KT-214/KT-217 (**được lead cho phép** trong phiếu KT-227) |
| `v2/agent/JOURNAL/kt227-layer2-fixes.md` | **mới** (tệp này) |
| `v2/agent/JOURNAL/kt227-logs/**` | **mới** |

**Không** chạm: `spec/`, `v2/agent/DECISIONS.md`, `docs/**`, `AGENTS.md`, journal của agent khác.
**Không** nới ngưỡng, **không** thêm allowlist để lấy xanh, **không** push, **không** `gate --fast`.

## Phụ lục — Cách tái lập

```bash
cd /private/tmp/night3-L53-kt224
export PATH="$HOME/.cargo/bin:$PATH"
python3 v2/tools/check-wake-paths.py --selftest          # 246/246
python3 v2/tools/check-wake-depinfo.py --selftest        # 27/27
python3 v2/agent/JOURNAL/kt227-logs/harness-kiem-lai.py  # 25 ca (≈15 phút)
./v2/bin/v2 check                                        # PASS
```
