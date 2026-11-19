# KT-209 — đóng hai lỗ hổng **tiềm ẩn** của checker `wake` (L40 KT-201 §D2 L2/L3)

**Tác giả**: L45 (sub-agent của Agent 1) · **Nhánh**: `night3/L45-kt209` · **Gốc**: `cfee57d8`
**Checker**: `v2/tools/check-wake-paths.py` · **Hợp đồng**: `v2/agent/reports/CHECKER-WAKE-CONTRACT.md`
**Nguồn lỗ hổng**: reviewer L40, `v2/agent/JOURNAL/kt201-review-vong4.md` §D2 (L2, L3).

---

## 0. KẾT LUẬN NGẮN

| Hạng mục | Trước | Sau |
|---|---|---|
| L2 — crate mới dưới `v2/kernel/`/`v2/sim/` chứa `wake` | **XANH (lọt)** | **ĐỎ** |
| L2 — `build.rs` của crate chứa `wake` | **XANH (lọt)** | **ĐỎ** |
| L3 — `include!`/`#[path]` trong `v2/lib` ngoài macro | **XANH (lọt)** | **ĐỎ** khi đích ra ngoài `v2/lib` |
| `--selftest` | 118/118 | **131/131** |
| `./v2/bin/v2 check` trên cây thật | PASS | **PASS** |
| Cây thật: checker thường + `--strict` | `errors=0` | **`errors=0`** |

Không nới ngưỡng, không thêm allowlist, không sửa `spec/`, không chạy `gate --fast`.

---

## 1. L2 — phạm vi luật `wake` hẹp hơn hợp đồng §1

### 1.1. Lỗ hổng (L40 §D2 L2)

`TEST_ONLY_WAKE_SCOPE` là **ba mục cứng** (`vifast-core/src/`, `vifast-sim/src/`,
`vifast-kbin-riscv64/src/`) + regex `vifast-plat-*/src/`. Hệ quả:

- `v2/kernel/vifast-newcrate/src/lib.rs` chứa `m.wake(t)` ⇒ **XANH**;
- `v2/sim/vifast-sim2/src/lib.rs` chứa `m.wake(t)` ⇒ **XANH**;
- `v2/kernel/vifast-core/build.rs` chứa `m.wake(t)` ⇒ **XANH** (chỉ `src/` được quét).

Hợp đồng §1 lại viết "`v2/kernel/**` → `wake` chỉ-dùng-test" — **sai** so với hành vi.

### 1.2. Bản vá — phạm vi theo **mẫu**

```python
RE_TEST_ONLY_WAKE_SCOPE = re.compile(r"^v2/(?:kernel|sim)/[^/]+/(?:src/.*\.rs|build\.rs)$")
```

Mọi crate dưới `v2/kernel/<crate>/` và `v2/sim/<crate>/` — **toàn bộ `src/**` cộng `build.rs`** —
nằm trong phạm vi, kể cả crate ra đời sau. `tests/` cấp crate vẫn ngoài (test-only,
`is_test_file` miễn). Không còn danh sách cứng, không còn regex riêng cho `vifast-plat-*`.

### 1.3. Đo **trước** khi mở rộng (bắt buộc — sợ cây thật ĐỎ)

Log: [`kt209-logs/03-scope-measure.log`](kt209-logs/03-scope-measure.log).

| Câu hỏi | Kết quả |
|---|---|
| Crate nào dưới `v2/kernel/`/`v2/sim/` có `src/` **ngoài** phạm vi cũ? | **0** (7/7 crate đã nằm trong phạm vi cũ) |
| Tệp nào **mới** vào phạm vi? | **1**: `v2/kernel/vifast-kbin-riscv64/build.rs` |
| Tệp nào **rơi khỏi** phạm vi? | **0** |
| Tệp mới vào phạm vi có token `wake`? | **0** ⇒ mở rộng **không** làm cây thật ĐỎ |
| Số tệp phạm vi cũ → mới | **129 → 130** |

**Ngoài mẫu** (đo được, ghi để không thành "phát hiện mới"): 26 tệp `.rs` **không thuộc crate** —
`v2/kernel/vifast-plat-user/lib.rs` (shim U-mode, biên dịch bởi `vifast-kbin-riscv64/build.rs`)
và 25 tệp `v2/kernel/vifast-kbin-riscv64/init/*.rs` (chương trình U-mode, biên dịch bởi
`build.rs`). **Cả 26 tệp: 0 token `wake`**; chúng không có `Machine` (chỉ syscall U-mode). Ghi ở
hợp đồng §4.

### 1.4. Đột biến chứng minh (cây thật, checker CŨ `cfee57d8` vs MỚI)

Log: [`kt209-logs/01-L2-mutation.log`](kt209-logs/01-L2-mutation.log).

| Ca | Tệp dựng | Checker CŨ | Checker MỚI |
|---|---|---|---|
| L2-a | `v2/kernel/vifast-newcrate/src/lib.rs` + `self.wake(t)` | `errors=0` (XANH) | **ĐỎ** `E_STRICT_TEST_ONLY_WAKE` |
| L2-b | `v2/sim/vifast-sim2/src/lib.rs` + `self.wake(t)` | `errors=0` | **ĐỎ** |
| L2-c | `v2/kernel/vifast-core/build.rs` + `self.wake(t)` | `errors=0` | **ĐỎ** |
| L2-d | `v2/sim/vifast-sim/build.rs` + `self.wake(t)` | `errors=0` | **ĐỎ** |
| L2-e | `v2/kernel/vifast-newcrate/src/sub/deep.rs` + `self.wake(t)` | `errors=0` | **ĐỎ** |
| L2-f | **đối chứng dương**: crate mới + `self.wake_from(t)` | `errors=0` | **XANH** |
| L2-g | **đối chứng dương**: `build.rs` + `self.wake_from(t)` | `errors=0` | **XANH** |

Đối chứng dương chứng minh luật **không** phải "thấy chữ `wake` là đỏ": `wake_from` (cửa sản
phẩm) vẫn xanh trong cùng phạm vi.

### 1.5. Ca selftest mới (7)

`KT-209/L2 crate kernel MỚI`, `crate sim MỚI`, `build.rs vifast-core`, `build.rs vifast-sim`,
`tệp con sâu trong crate MỚI` (đỏ) + `ĐỐI CHỨNG DƯƠNG crate kernel MỚI (wake_from)`,
`ĐỐI CHỨNG DƯƠNG build.rs (wake_from)` (xanh).

### 1.6. Hợp đồng §1

Viết lại bảng phạm vi theo **mẫu** (`<crate>/src/**` + `build.rs`), bỏ câu "`v2/kernel/**`" sai,
ghi rõ 26 tệp không thuộc crate nằm ngoài mẫu (kèm số đo 0 token `wake`).

---

## 2. L3 — `include!` trong `v2/lib` ngoài macro không được phân giải

### 2.1. Lỗ hổng (L40 §D2 L3)

`resolve_include_targets` chỉ seed worklist từ `v2/kernel` + `v2/sim`. `include!` trong `v2/lib`
**ngoài** thân macro không được phân giải ⇒ đích ngoài cây không bị quét. Hợp đồng §4 chỉ ghi
`#[path]`, **không** ghi `include!`.

### 2.2. Bản vá — seed thêm `v2/lib`, luật đích **theo cây nguồn**

- Worklist seed thêm **mọi** tệp `v2/lib/**/*.rs` (`is_test=False` — `test_only_modules` chỉ dựng
  đồ thị module cho kernel/sim).
- Nguồn `v2/kernel`/`v2/sim`: đích phải trong `v2/kernel`+`v2/sim` (**không đổi**).
- Nguồn `v2/lib`: đích phải trong **`v2/lib`**; sang `v2/kernel`/`v2/sim` hoặc ra ngoài ⇒ **ĐỎ**
  (`E_STRICT_WAKE_INCLUDE`, fail-closed).
- Đích trong `v2/lib` được phân giải **tiếp** (include! lồng không trốn được) nhưng **không** vào
  `extra` — `extra` là tập bị luật `wake` quét; mã `v2/lib` không có `Machine` nên `wake()`
  không thể xuất hiện ở đó, và cạnh sang kernel/sim đã bị chặn.

**Vì sao chọn "đích phải trong `v2/lib`" (không phải "đích trong bất kỳ cây nào đã quét")**:
`include!`/`#[path]` của lib kéo mã **tầng khác** vào crate lib — đó là cạnh nguồn ẩn đúng nghĩa
(mã kernel biên dịch trong ngữ cảnh lib, ngoài đồ thị module của kernel). Fail-closed là hướng
duy nhất không phải tin vào "đích kia chắc đã được quét ở chỗ khác". Đây cũng là cách kiểm mà
đề bài yêu cầu: đột biến `v2/lib/vifast-abi/src/lib.rs` +
`include!("../../../kernel/vifast-core/src/evil.rs")` ⇒ **ĐỎ**.

### 2.3. Đo **trước** khi sửa (sợ báo oan)

| Câu hỏi | Kết quả |
|---|---|
| `include!` trong `v2/lib` (ngoài macro) trên cây thật? | **0** |
| `#[path]` trong `v2/lib` trên cây thật? | **1**: `v2/lib/vifast-idl/src/lib.rs:32` → `generated_pager.rs` (**trong** `v2/lib`) |
| Đích của `#[path]` đó có tồn tại? | **Có** (`v2/lib/vifast-idl/src/generated_pager.rs`) |
| Có token `wake` trong `v2/lib`? | **0** (chỉ `Msg::Wake`/`SYS_WAKE_ADDR` — hoa) |

⇒ Bản vá **không** báo oan trên cây thật: checker thường + `--strict` vẫn `errors=0`
([`kt209-logs/04-real-tree-checker.log`](kt209-logs/04-real-tree-checker.log)).

### 2.4. Đột biến chứng minh (cây thật, checker CŨ vs MỚI)

Log: [`kt209-logs/02-L3-mutation.log`](kt209-logs/02-L3-mutation.log).

| Ca | Dựng | Checker CŨ | Checker MỚI |
|---|---|---|---|
| L3-a | `v2/lib/vifast-abi/src/lib.rs` + `include!("../../../kernel/vifast-core/src/evil.rs")`, `evil.rs` **sạch** | `errors=0` (XANH) | **ĐỎ** `E_STRICT_WAKE_INCLUDE` (cạnh nguồn, không phải nội dung `wake`) |
| L3-b | như L3-a nhưng `evil.rs` **chứa** `self.wake(t)` | — | **ĐỎ** (2 lỗi: cạnh + `wake` trong `evil.rs`) |
| L3-c | `include!("../../../../tmp/kt209/evil_outside.rs")` (ra ngoài cây) | `errors=0` | **ĐỎ** |
| L3-d | **đối chứng dương**: `include!("kt209_ok.rs")` (đích trong `v2/lib`) | — | **XANH** |
| L3-e | **lồng**: `lib.rs` → `kt209_nested_a.rs` (trong lib) → `/tmp/.../evil_outside.rs` | — | **ĐỎ** |
| L3-f | `#[path = "../../../kernel/vifast-core/src/evil.rs"] mod evil;` | `errors=0` | **ĐỎ** |
| L3-g | **đối chứng dương**: `#[path = "kt209_ok.rs"] mod kt209_ok;` | — | **XANH** |

L3-a là ca **tách bạch**: đích `evil.rs` **không** chứa `wake`, nên nếu checker chỉ bắt nội dung
`wake` thì ca này XANH; nó ĐỎ chứng minh **cạnh nguồn** bị kiểm. L3-b cho thấy khi đích có `wake`
thì có **hai** lý do đỏ (cạnh + nội dung) — không thay thế nhau.

### 2.5. Ca selftest mới (6)

`include! lib → kernel (đích sạch) — cạnh nguồn bị kiểm`, `include! lib → ngoài cây`,
`include! LỒNG lib → lib → ngoài cây`, `#[path] lib → kernel` (đỏ) +
`include! lib → lib (đối chứng dương)`, `#[path] lib → lib (đối chứng dương)` (xanh).

### 2.6. Hợp đồng §2/§3/§4/§5

- §2: thêm dòng `include!`/`#[path]` trong `v2/lib` ngoài macro được phân giải (đệ quy hết lớp).
- §3: thêm dòng từ chối "đích ra ngoài `v2/lib` (kể cả sang kernel/sim)".
- §4: đóng dòng "`#[path]` trong `v2/lib`"; thêm dòng "tệp `.rs` không thuộc crate" (26 tệp,
  0 token `wake`).
- §5: "ba đường" thay vì "hai đường" (thêm phân giải `include!`/`#[path]` trong `v2/lib`); ghi
  `build.rs` **nằm trong** phạm vi luật `wake` từ KT-209/L2.

---

## 3. Chất lượng

### 3.1. Selftest

```
$ python3 v2/tools/check-wake-paths.py --selftest
# selftest: 131/131 ok
```

**118 → 131** (+13 ca: 7 L2 + 6 L3). Không mất ca nào; 118 ca cũ vẫn xanh nguyên.
Log: [`kt209-logs/05-selftest.log`](kt209-logs/05-selftest.log).

### 3.2. Cây thật

```
$ python3 v2/tools/check-wake-paths.py            # errors=0
$ python3 v2/tools/check-wake-paths.py --strict   # errors=0
```

Log: [`kt209-logs/04-real-tree-checker.log`](kt209-logs/04-real-tree-checker.log).

### 3.3. `./v2/bin/v2 check`

```
v2 check: PASS (L1 chạy trong gate --fast)
```

Log: [`kt209-logs/06-final-v2check.log`](kt209-logs/06-final-v2check.log). Baseline trước bản vá:
[`kt209-logs/00-baseline-v2check.log`](kt209-logs/00-baseline-v2check.log) (PASS).

**Không** chạy `gate --fast` (bị cấm trong vòng task). **Không** gặp T-INFRA SIGTRAP ở bước
`luật cấu trúc`.

---

## 4. Giới hạn còn lại (trung thực)

1. **Tệp `.rs` không thuộc crate** — `v2/kernel/vifast-plat-user/lib.rs` + 25 tệp
   `v2/kernel/vifast-kbin-riscv64/init/*.rs` nằm **ngoài** mẫu `<crate>/src/**`/`build.rs` nên
   **không** bị luật `wake` quét **trực tiếp**. Đo 2026-11-20: **0** token `wake`; chúng là
   shim/chương trình U-mode, không có `Machine`. (Một số tệp `init/*.rs` — vd
   `init/pager_common.rs` — được kéo vào qua `#[path]` từ `src/` và xử lý như đích `extra`.)
   Nếu muốn phủ hết, cần mở mẫu thành "mọi `.rs` sản phẩm dưới `v2/kernel`/`v2/sim` trừ
   `tests/`" — **không** làm trong task này vì đề bài chốt mẫu `<crate>/src/**` + `build.rs`.
2. **`include!` trong `v2/lib/*/tests/*.rs`** được đối xử như **sản phẩm** (seed `is_test=False`)
   ⇒ nếu một test lib include tệp ngoài `v2/lib` thì ĐỎ. Cây thật **0** ca như vậy; đây là
   fail-closed có chủ đích, ghi để người sau biết.
3. **Đệ quy vào đích `v2/lib`** là lớp dự phòng: vì mọi tệp `v2/lib/**/*.rs` đã được seed trực
   tiếp, ca lồng vẫn bị bắt kể cả khi không có đệ quy (L3-e bắt ở tệp trung gian). Giữ đệ quy để
   nếu sau này thu hẹp seed thì `include!` lồng vẫn không trốn.
4. **Macro invocation** (`cfg_if! { … }`) vẫn ngoài phạm vi — như hợp đồng §4 đã ghi từ trước,
   **không** phải hồi quy của task này.
5. **Không** kiểm `gate --fast`, L1/sim/Miri/soak — ngoài phạm vi luật này (đề bài cấm
   `gate --fast`).

---

## 5. Tệp thay đổi

| Tệp | Thay đổi |
|---|---|
| `v2/tools/check-wake-paths.py` | L2: mẫu phạm vi; L3: seed `v2/lib` + luật đích theo cây nguồn; +13 ca selftest; docstring |
| `v2/agent/reports/CHECKER-WAKE-CONTRACT.md` | §1/§2/§3/§4/§5 theo hành vi mới |
| `v2/agent/JOURNAL/kt209-scope-gaps.md` | journal này |
| `v2/agent/JOURNAL/kt209-logs/*` | log thô (đo phạm vi, đột biến, selftest, `v2 check`) |

**Không** chạm `spec/`, `v2/agent/QUEUE.md`, `v2/agent/DECISIONS.md`,
`docs/QUYET-DINH-sau-review.md`, `docs/v2/*`, `docs/v1/*`, `AGENTS.md`, `v2/AGENTS.md`.
