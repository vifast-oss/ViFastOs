# PHIẾU GỬI AGENT 0 — nghiệm thu KT-241 (API `NonReadyTarget`)

**Từ**: Agent 2 dòng OS (subagent `kt241-accept`), theo lệnh lead.
**Ngày phiếu**: 2026-11-21 (chạy thật đêm 2026-11-20 19:14–20:06 JST).
**Nhánh**: `task/kt241-accept`, worktree `/tmp/kt242-s1`, tách từ `main` @ **`c6127fa0`**.
**Phạm vi**: khép nghiệm thu KT-241 — ma trận tấn công, kiểm cây, cập nhật hồ sơ.
**Không** push, **không** merge — chờ lead review.

---

## 1. Trạng thái một khối

```text
HEAD (main)       c6127fa0   [KT-242] hồ sơ: lưu log cổng + K6 + checksum
worktree          /tmp/kt242-s1   nhánh task/kt241-accept
toolchain         Rust 1.100.0 (0ef1bc386 2026-11-09) — ĐÃ đặt RUSTUP_TOOLCHAIN=1.100.0
build-inputs      54f3b59fc08f3221 (234 mục, algo kt134-v4, dirty=0)
ảnh sản phẩm      d915b977edbdae4661a3bb31e4aa7ec33f8e9d703bbba2c7f88069f200e541dc (191 288 B)
gate --fast       PASS 53/53, 684,64 s — log JOURNAL/kt242-logs/kt242-gate-main.log
ma trận KT-241    28/28 PASS, 0 ĐỎ, 0 ERROR
cây sau ma trận   SẠCH (không tệp mã sản phẩm / checker nào bị sửa)
WB-16 / WB-17     CLOSED ở tầng API (A1–A4 + B1–B5 PASS)
WB-19             OPEN — lỗ tầng CHECKER, luồng 2 đang vá (không thuộc KT-241)
KT-241            DONE
```

---

## 2. Ma trận tấn công — 28 ô

Chạy **từng ô**, `--with-depinfo` (đủ ba lớp: l1 tĩnh, l1s `--strict`, l2 dep-info), mỗi ô có **đối chứng dương**.
Lệnh: `RUSTUP_TOOLCHAIN=1.100.0 bash v2/agent/JOURNAL/kt241-attack/run.sh --cell <ID> --with-depinfo`

**Nền (cây sạch) trước khi chạy**: l1 `errors=0`, l1s `errors=0`, l2 `errors=0` (59 tệp) — `logs/baseline-*.log`.

| Ô | Nhóm | Kết quả | Bằng chứng |
|---|---|---|---|
| A1 | A | **PASS** | `logs/A1/` — cargo `E0308`; l1/l1s `E_STRICT_TRANSITION`; l2 `E_DEPINFO_*`; dương PASS |
| A2 | A | **PASS** | `logs/A2/` — cargo `E0308`; l1/l1s `E_STRICT_TRANSITION`; l2 `E_DEPINFO_*`; dương PASS |
| A3 | A | **PASS** | `logs/A3/` — cargo `E0308,E0631`; l2 `E_DEPINFO_BUILD,SCOPE`; dương PASS |
| A4 | A | **PASS** | `logs/A4/` — cargo `E0308`; l1/l1s `E_STRICT_TRANSITION`; l2 `E_DEPINFO_*`; dương PASS |
| B1 | B | **PASS** | `logs/B1/` — cargo `E0308` `expected NonReadyTarget, found TcbState`; l2 `E_DEPINFO_*`; dương PASS |
| B2 | B | **PASS** | `logs/B2/` — cargo `E0308`; l2 `E_DEPINFO_*`; dương PASS |
| B3 | B | **PASS** | `logs/B3/` — cargo `E0308`; l2 `E_DEPINFO_*`; dương PASS |
| B4 | B | **PASS** | `logs/B4/` — cargo `E0308`; l2 `E_DEPINFO_*`; dương PASS |
| B5 | B | **PASS** | `logs/B5/` — cargo `E0308`; l2 `E_DEPINFO_*`; dương PASS |
| B6 | B | **PASS** | `logs/B6/` — đối chứng dương: cargo OK, ba lớp XANH |
| C1 | C | **PASS** | `logs/C1/` — l1/l1s `E_STRICT_TEST_ONLY_WAKE`; l2 `E_DEPINFO_WAKE`; dương PASS |
| C2 | C | **PASS** | `logs/C2/` — l1/l1s `E_STRICT_TEST_ONLY_WAKE`; l2 `E_DEPINFO_WAKE`; dương PASS |
| C3 | C | **PASS** | `logs/C3/` — l1/l1s `E_STRICT_TEST_ONLY_WAKE`; l2 `E_DEPINFO_WAKE`; dương PASS |
| C4 | C | **PASS** | `logs/C4/` — l1/l1s `E_STRICT_WAKE_INCLUDE`; l2 `E_DEPINFO_WAKE`; dương PASS |
| C5 | C | **PASS** | `logs/C5/` — l1/l1s `E_STRICT_TEST_ONLY_WAKE`; l2 XANH (đúng kỳ vọng); dương PASS |
| D1 | D | **PASS** | `logs/D1/` — l2 `E_DEPINFO_WAKE`; dương PASS |
| D2 | D | **PASS** | `logs/D2/` — l1/l1s `E_STRICT_TEST_ONLY_WAKE`; l2 `E_DEPINFO_WAKE`; dương PASS |
| D3 | D | **PASS** | `logs/D3/` — l1/l1s `E_STRICT_TEST_ONLY_WAKE`; l2 `E_DEPINFO_WAKE`; dương PASS |
| D4 | D | **PASS** | `logs/D4/` — l2 `E_DEPINFO_WAKE`; dương PASS |
| D5 | D | **PASS** | `logs/D5/` — l1/l1s `E_STRICT_TEST_ONLY_WAKE`; l2 `E_DEPINFO_WAKE`; dương PASS |
| **D6** | D | **PASS** | `logs/D6/` — **ô MỚI**: l1/l1s `E_STRICT_WAKE_INCLUDE`; l2 `E_DEPINFO_SCOPE,E_DEPINFO_WAKE`; dương PASS |
| E1 | E | **PASS** | `logs/E1/` — l1/l1s `E_STRICT_NONREADY_CONV,TRANSITION`; l2 `E_DEPINFO_*`; dương PASS |
| E2 | E | **PASS** | `logs/E2/` — l1/l1s `E_STRICT_NONREADY_CONV,TRANSITION`; l2 `E_DEPINFO_*`; dương PASS |
| E3 | E | **PASS** | `logs/E3/` — cargo `E0624`; l1/l1s `E_STRICT_STATE_WRITE`; l2 `E_DEPINFO_BUILD`; dương PASS |
| E4 | E | **PASS** | `logs/E4/` — cargo `E0308`; l2 `E_DEPINFO_BUILD`; dương PASS |
| E5 | E | **PASS** | `logs/E5/` — cargo `E0624`; l1/l1s `E_STRICT_STATE_WRITE`; l2 `E_DEPINFO_BUILD`; dương PASS |
| E6 | E | **PASS** | `logs/E6/` — cargo `unsafe`; l2 `E_DEPINFO_BUILD`; dương PASS |
| E7 | E | **PASS** | `logs/E7/` — l1/l1s `E_ANCHOR_TYPE` (`state_write.rs:150`); l2 `E_DEPINFO_BUILD`; dương PASS |

Bảng gộp: [`JOURNAL/kt241-attack/results/current.tsv`](v2/agent/JOURNAL/kt241-attack/results/current.tsv).
Bảng 27 ô **trước** khi sửa kỳ vọng (giữ làm lịch sử): `results/current-27-truoc-fix.tsv`.

**Tổng: 28 PASS / 0 ĐỎ / 0 ERROR.**

---

## 3. Ba ô đỏ cũ = **lệch kỳ vọng**, đã sửa + thêm ô D6

Lần chạy đầu, C3/D5/E7 **ĐỎ** dù tấn công **bị bắt** ở cả ba lớp và đối chứng dương PASS. Nguyên nhân
là **mã lỗi checker không khớp regex kỳ vọng của `manifest.tsv`** — lỗi **kỳ vọng**, không phải lỗ hổng.

### 3.1 C3 / D5 — `#[path]` trỏ ra ngoài `src/` (WB-07 / WB-08)

Đích `vifast-core/evil.rs` nằm **trong** `v2/kernel` ⇒ theo luật B6/B7 nó được **quét** (rồi luật
`wake` bắt `E_STRICT_TEST_ONLY_WAKE`). `E_STRICT_WAKE_INCLUDE` **chỉ** phát khi `_inside_scan_tree()`
trả `None`, tức đích **ngoài** `v2/kernel`+`v2/sim` (`check-wake-paths.py:2011–2016`). Mô tả ô ghi
"trỏ ra ngoài `src/`" nhưng luật kiểm là "trỏ ra ngoài **cây quét**" ⇒ **mô tả ô không khớp nội dung ô**.

### 3.2 E7 — đột biến chữ ký `transition_tcb`

Checker phát `E_ANCHOR_TYPE` (mỏ neo **sai kiểu**), đúng thiết kế: *"ĐỎ khi **thiếu**
(`E_ANCHOR_MISSING`) hoặc **sai** (`E_ANCHOR_TYPE`)"*. Manifest thiếu `E_ANCHOR_TYPE`.

### 3.3 Đã sửa (quyết định lead: hướng **A** — sửa kỳ vọng, **không** siết luật sản phẩm)

| Ô | `expect_codes_checker` trước | sau |
|---|---|---|
| C3 | `E_STRICT_WAKE_INCLUDE\|E_DEPINFO_WAKE` | `E_STRICT_TEST_ONLY_WAKE\|E_STRICT_WAKE_INCLUDE\|E_DEPINFO_WAKE` |
| D5 | `E_STRICT_WAKE_INCLUDE\|E_DEPINFO_WAKE` | `E_STRICT_TEST_ONLY_WAKE\|E_STRICT_WAKE_INCLUDE\|E_DEPINFO_WAKE` |
| E7 | `E_STRICT_\|E_ANCHOR_MISSING` | `E_STRICT_\|E_ANCHOR_MISSING\|E_ANCHOR_TYPE` |

**KHÔNG nới tiêu chí**: `expect_l1`/`expect_l1s`/`expect_l2` của cả ba ô **giữ nguyên**
`red`/`red`/`red` (E7: `red`/`red`/`any`). Mỗi ô vẫn đòi **ĐỎ ở cả ba lớp**; chỉ mở rộng **tập mã
lỗi được chấp nhận** cho khớp luật đã hiện thực. Ghi chú: bản sửa C3/D5 **giữ** `E_DEPINFO_WAKE`
trong regex (lead nêu `E_STRICT_TEST_ONLY_WAKE|E_STRICT_WAKE_INCLUDE`, nhưng regex dùng chung cho
**cả ba lớp** nên thiếu `E_DEPINFO_WAKE` sẽ làm lớp 2 ĐỎ oan).

### 3.4 Ô mới **D6** — kiểm đúng "trỏ ra ngoài **cây quét**"

```rust
// v2/kernel/vifast-core/src/kt241_d6.rs
#[path = "../../../agent/JOURNAL/kt241-attack/mock/d6_evil.rs"]
mod evil;
```

Đích `v2/agent/JOURNAL/kt241-attack/mock/d6_evil.rs` **ngoài** `v2/kernel`+`v2/sim`, chứa `m.wake(t)`.
Kết quả **đúng như kỳ vọng**:

```text
l1  E E_STRICT_WAKE_INCLUDE  kt241_d6.rs:6: `#[path]` trỏ ra ngoài `v2/kernel`+`v2/sim` (fail-closed, KT-171 B6/B7)
l2  E E_DEPINFO_SCOPE        mock/d6_evil.rs: tệp rustc nạp KHÔNG thuộc crate nào (fail-closed, KT-238)
l2  E E_DEPINFO_WAKE         mock/d6_evil.rs:6: gọi `wake(` trong mã sản phẩm (D2-144)
```

**Vì sao không dùng `v2/lib` như lead gợi ý**: (i) **không** tệp nào trong `v2/lib` chứa `wake`
(đã grep toàn bộ) ⇒ lớp 2 sẽ **XANH**, không đạt kỳ vọng `E_DEPINFO_WAKE`; (ii) `vifast-alloc/src/lib.rs`
có `#![no_std]`/`#![forbid(unsafe_code)]` nên **không biên dịch được** khi nạp làm module ⇒ cargo ĐỎ,
lớp 2 ra `E_DEPINFO_BUILD`. Mock fixture ngoài cây quét đạt **đúng cả hai** kỳ vọng và vẫn là
"đường tương đương trỏ ra ngoài cây quét". Đây là **tăng phủ**, không phải nới.

---

## 4. Cây sau ma trận

```text
$ git status --porcelain | grep -vE '^(A |M |MM )?(PHIEU-GUI-AGENT0-2026-11-21-KT241\.md|v2/agent/(QUEUE|BYPASS-REGISTRY)\.md|v2/agent/JOURNAL/kt241-attack/)'
(không có dòng nào)
```

Script có trap `EXIT/INT/TERM` khôi phục từ backup riêng (**không** dùng `git checkout/reset`).
Đã kiểm sau **từng** ô: không tệp mã sản phẩm nào bị sửa; `v2/kernel/vifast-core/evil.rs` và các
tệp chèn tạm đều đã biến mất. **Không** sửa `checker` (`check-wake-paths.py`/`check-wake-depinfo.py`
nguyên trạng — luồng 2 đang vá).

---

## 5. Cổng và ảnh

| Hạng mục | Kết quả | Bằng chứng |
|---|---|---|
| `gate --fast` trên `main` @ `c6127fa0` | **PASS 53/53**, 684,64 s | `JOURNAL/kt242-logs/kt242-gate-main.log` |
| build-inputs | `54f3b59fc08f3221`, 234 mục, `algo=kt134-v4`, `dirty=0` | `JOURNAL/kt242-logs/README.md` §1 |
| ảnh sản phẩm | `d915b977…e541dc`, 191 288 B | `JOURNAL/kt242-logs/README.md` §1 |
| K6 — ma trận boot | 21/21 boot sạch | `kt242-logs/ktest-matrix.log` |
| K6 — persist | PASS 7/7 pha | `kt242-logs/persist.log` |
| K6 — conformance | 61/61 dòng khớp | `kt242-logs/conformance.log` |
| K6 — soak | 1000/1000, 0 lỗi | `kt242-logs/soak.log` |
| K6 — L1 400k (smp 1/2/4/8) | 4/4 PASS | `kt242-logs/l1-smp{1,2,4,8}.log` |

**Gate không chạy lại trong worktree**: HEAD worktree **trùng** `c6127fa0` với `main`, và toàn bộ
thay đổi của phiếu này là **tài liệu + log + manifest ma trận** (không chạm `crates/`, `v2/kernel`,
`v2/sim`, `v2/lib`, `v2/tools/check-*.py`) ⇒ cổng đã xanh trên đúng cây đó. Nếu lead muốn bằng
chứng cổng **trong worktree**: `cd /tmp/kt242-s1 && ./v2/bin/v2 gate --fast` (~11 phút).

---

## 6. WB-16 / WB-17 / WB-19

| Mã | Tầng | Cơ chế | Ô chứng minh | Trạng thái |
|---|---|---|---|---|
| **WB-16** | **API** | `Ready` qua helper liên hàm / `const` / `static` / closure / hàm trả về | **B1–B5** | **CLOSED** — `E0308` `expected NonReadyTarget, found TcbState` |
| **WB-17** | **API** | `transition_tcb` làm fn-pointer / generic `F: Fn` / trait object / UFCS | **A1–A4** | **CLOSED** — `E0308`/`E0631` |
| **WB-19** | **checker** | `impl`/`fn` **lồng trong thân cửa** gọi thẳng `write_state` ⇒ cả hai lớp XANH | — | **OPEN** — luồng 2 đang vá |

**Đọc kỹ phạm vi**: WB-16/WB-17 đóng ở tầng **API** (đường gọi sai **không biểu diễn được**).
Điều này **KHÔNG** có nghĩa "luật `wake` đã đóng" — **WB-19** là lỗ ở tầng **checker** và **vẫn OPEN**.
Không suy từ "WB-16/17 CLOSED" ra "checker kín".

---

## 7. Việc còn lại

1. **Commit** — xem §9 (hook chặn agent, lead commit).
2. **WB-19** — luồng 2 vá; khi xong cần ô ma trận riêng cho "cửa thứ bảy" (ngoài phạm vi KT-241).
3. **`v2/agent/STATE.md` đang cũ** — vẫn ghi `HEAD a2e18cdf`, `build-inputs 233/43df126a`,
   ảnh `639ca6d1…`, tức **trước KT-242**. Tôi **không** sửa: (a) ngoài phạm vi phiếu, (b) `main`
   đang có thay đổi **chưa commit** ở chính tệp này nên sửa từ nhánh sẽ xung đột. Đề nghị lead
   cập nhật STATE sau khi tích hợp KT-242 + phiếu này.

**Câu hỏi cho A0**: không còn câu hỏi chặn. Hai điểm để A0 biết khi phê chuẩn:
(a) regex C3/D5 có thêm `E_DEPINFO_WAKE` (bắt buộc, vì regex dùng chung ba lớp);
(b) D6 dùng mock fixture ngoài cây quét thay cho `v2/lib` (lý do ở §3.4).

---

## 8. Tệp đã đổi trong nhánh này

| Tệp | Thay đổi |
|---|---|
| `v2/agent/QUEUE.md` | dòng KT-241: `READY` → **`DONE`** + khối kết quả 28/28 |
| `v2/agent/BYPASS-REGISTRY.md` | WB-16/17 → `CLOSED` **ở tầng API**; **thêm WB-19** (`OPEN`, tầng checker); mục "ba đường" ghi rõ hai tầng |
| `v2/agent/JOURNAL/kt241-attack/manifest.tsv` | 3 dòng sửa kỳ vọng (C3, D5, E7) + **1 dòng thêm D6** + header 27→28 ô |
| `v2/agent/JOURNAL/kt241-attack/cells/D6*.txt` | payload ô D6 (tấn công + đối chứng dương) |
| `v2/agent/JOURNAL/kt241-attack/mock/d6_evil.rs` | đích ngoài cây quét cho D6 |
| `v2/agent/JOURNAL/kt241-attack/results/current.tsv` | bảng gộp **28 ô** |
| `v2/agent/JOURNAL/kt241-attack/results/current-27-truoc-fix.tsv` | bảng 27 ô trước khi sửa (lịch sử) |
| `v2/agent/JOURNAL/kt241-attack/logs/<ID>/` | log từng ô (cargo, l1, l1s, l2, dương) |
| `v2/agent/JOURNAL/kt241-attack/logs/baseline-*.log` | nền cây sạch ba lớp |
| `v2/agent/JOURNAL/kt241-attack/drive-remaining.sh`, `drive-fix.sh` | script chạy ô tuần tự + gộp bảng |
| `PHIEU-GUI-AGENT0-2026-11-21-KT241.md` | phiếu này |

**Không** chạm `spec/`, `docs/QUYET-DINH-sau-review.md`, `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/`,
mã sản phẩm, hay checker.

---

## 9. Commit — **CHƯA COMMIT ĐƯỢC**, chờ lead

Toàn bộ thay đổi **đã stage** trong worktree, `HEAD` vẫn là `c6127fa0`. Commit bị **`vifast-guard`**
(pre-commit) **TỪ CHỐI**:

```text
vifast-guard(pre-commit): TỪ CHỐI COMMIT — KT-85 / D2-123 mục (4)(b),(d)
  Lý do: chưa đặt VIFAST_ROLE ⇒ fail-closed
  vifast-guard: RESULT=DENY hook=pre-commit role=(chưa đặt)
```

Hook ghi rõ **chỉ Agent 1 (lead)** đặt được `VIFAST_ROLE`. Tôi **không** đặt `VIFAST_ROLE=lead`
(mạo danh lead, vô hiệu hoá chốt chặn có chủ đích) và **không** dùng `--no-verify` (quyền của
người thật). Đây là **đúng luồng** hook thiết kế: agent dựng worktree riêng + stage, **lead commit**.

**Lead chạy** (thông điệp đã soạn sẵn ở `/tmp/kt241-commit-msg.txt`):

```bash
VIFAST_ROLE=lead git -C /tmp/kt242-s1 commit -F /tmp/kt241-commit-msg.txt
```

Nhãn commit đã soạn: `[KT-241] ma trận tấn công 28/28 + nghiệm thu: sửa kỳ vọng manifest + ô D6`.
Sau đó lead review rồi merge/cherry-pick sang `main` (tôi **không** push, **không** merge).
