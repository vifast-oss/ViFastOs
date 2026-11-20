# KT-238 — phạm vi bảo vệ sinh từ `cargo metadata`; đóng WB-18

- **Người làm**: sub-agent B của Agent 1 (task KT-238, luồng B theo **D2-192**).
- **Worktree**: `/private/tmp/night4-B-kt238`, nhánh `night4/B-kt238`, base `298b92c5`.
- **Ngân sách**: 3 giờ. **Cấm**: push, `gate --fast`, sửa `spec/`/`QUEUE.md`/`DECISIONS.md`/`docs/**`/
  `AGENTS.md`/journal người khác.
- **Quyết định nguồn**: D2-188 (Agent 0) — 5 chốt; D2-193 §2 (WB-18 là P0 chặn nghiệm thu).
- **Tệp sửa**: `v2/tools/check-wake-depinfo.py` (**không** chạm `check-wake-paths.py` — luồng C).
- **Log**: `v2/agent/JOURNAL/kt238-logs/`.

---

## 0. Kết luận trước

| Hạng mục | Kết quả |
|---|---|
| Phạm vi sinh từ `cargo metadata` | **22 crate bảo vệ + 1 crate từ chối rõ** trên cây thật (23/1 khi có crate repro) |
| WB-18 trước | lớp 1 XANH (thường + `--strict`), lớp 2 **XANH** chỉ `W_DEPINFO_SKIPPED` |
| WB-18 sau | lớp 2 **ĐỎ** `E_DEPINFO_WAKE` trên `…/out/evil.rs` (crate nằm **trong phạm vi bảo vệ**) |
| Đối chứng dương | crate mới `v2/extra/ok` (cùng cơ chế `build.rs`→`$OUT_DIR`→`include!`, mã lành) ⇒ lớp 2 **XANH** |
| Fail-open còn lại | **0** đã biết trong phạm vi lớp 2 (xem §4) |
| `OUT_DIR` đúng lần build | **có** — target dir từ metadata + artifact/fingerprint + dấu sha256 cho `--no-build` (§5) |
| Selftest lớp 2 | **52/52** (37 cũ + 15 mới) |
| Selftest lớp 1 | **295/295** (không mất ca) |
| `./v2/bin/v2 check` | **PASS** |

---

## 1. Vì sao phải sinh phạm vi từ `cargo metadata`

WB-18 (KT-233) không phải "một biến thể tên tệp" mà là **lỗi phạm vi bảo vệ**: lớp 1 quét theo ba
gốc cứng (`v2/kernel`, `v2/sim`, `v2/lib`), lớp 2 bỏ mọi tệp trong thư mục `target/`. Giao của hai
vùng mù là **`$OUT_DIR` của một crate thành viên nằm ngoài ba gốc** — `cargo check` RC=0, cả hai
lớp XANH, `v2 check` PASS.

Ba đường dẫn cố định **không phải** định nghĩa đầy đủ của phạm vi: crate thêm sau (`v2/extra/…`),
crate chỉ tới được qua path-dependency (`v2/kernel/vifast-core` được `v2/sim/vifast-sim` trỏ vào),
và crate công cụ (`v2/tools/vifast-agentctl`) đều không suy ra được từ ba đường dẫn ấy.

---

## 2. Phạm vi sinh từ `cargo metadata` (D2-188 chốt 1)

### 2.1 Cách sinh

`load_scope()` chạy `cargo metadata --no-deps --format-version 1` cho **hai** workspace sản phẩm
(`v2/Cargo.toml`, `v2/kernel/Cargo.toml`), rồi lặp **bao đóng path-dependency nội bộ**: mọi
`dependencies[].path` trỏ vào trong repo mà chưa có trong tập crate ⇒ chạy metadata trên
`<dir>/Cargo.toml` của nó, tới điểm bất động (tối đa 8 vòng).

Từ mỗi `packages[]` lấy: `name`, `manifest_path` (⇒ `dir_rel`), `targets[].src_path`,
`targets[].name`, `dependencies[].path`. `target_directory` của mỗi workspace được gắn cho đơn vị
build tương ứng (`host`/`kernel`) — **đây là target dir dùng để đọc `.d`**, không phải hằng
`v2/target`/`v2/kernel/target`.

### 2.2 Phân loại (bảo vệ / từ chối rõ) và lý do

| Loại | Tiêu chí | Lý do |
|---|---|---|
| **Bảo vệ** | mọi crate trong workspace + bao đóng path-dep nội bộ, trừ diện từ chối | sản phẩm: crate có thể chứa/được nạp mã gọi `wake`/`transition_tcb` |
| **Từ chối rõ** | `dir_rel` dưới `v2/tools/` **và** bao đóng dependency **không** chứa crate bảo vệ nào | công cụ điều khiển, không phải sản phẩm; không thể tham chiếu API `wake` vì không phụ thuộc crate sản phẩm |
| **Nâng lên bảo vệ** | crate `v2/tools/` **có** crate bảo vệ trong bao đóng dependency (kể cả dev/build-dep) | fail-closed: công cụ phụ thuộc sản phẩm **có thể** gọi API `wake` ⇒ không được miễn |

Cây thật (sau khi dọn repro): **22 bảo vệ, 1 từ chối rõ** — `vifast-agentctl`
(`v2/tools/vifast-agentctl`), lý do "công cụ (v2/tools/), không phải sản phẩm", bao đóng
dependency rỗng nên không bị nâng. Danh sách đầy đủ in ở đầu mỗi lần chạy
(`# phạm vi (cargo metadata): …`).

### 2.3 Tệp không quy được về crate nào

- tệp rustc nạp **không** thuộc crate nào trong metadata và **không** phải tệp sinh trong target
  dir ⇒ `E_DEPINFO_SCOPE` (**từ chối rõ**), **vẫn quét R1** (fail-closed hai lớp);
- tệp sinh trong target dir mà **không** quy được về crate (artifact không có `package_id` path,
  tên `.d` không khớp target name) ⇒ `E_DEPINFO_SCOPE`;
- tệp dưới một thư mục tên `target` **không phải** target dir của metadata ⇒ `E_DEPINFO_SCOPE`
  (trước KT-238: bị lọc im lặng).

---

## 3. Đóng WB-18 (D2-188 chốt 2, 3)

### 3.1 Dựng lại repro

```
v2/extra/evil/Cargo.toml   # name = "vifast-evil"; dep path vifast-sim
v2/extra/evil/build.rs     # ghi $OUT_DIR/evil.rs = fn evil(m,t){ m.wake(t) }
v2/extra/evil/src/lib.rs   # include!(concat!(env!("OUT_DIR"), "/evil.rs"));
v2/Cargo.toml              # members += "extra/evil"
```

`cargo check -p vifast-evil` ⇒ **RC=0**. `.d` của unit **có** liệt kê tệp sinh (bằng chứng:
`kt238-logs/wb18-depinfo-evidence.log`):

```
v2/target/debug/deps/vifast_evil-128b1201c00c2cfb.d:
  extra/evil/src/lib.rs /…/v2/target/debug/build/vifast-evil-21dcaed65d956140/out/evil.rs
```

### 3.2 Trước / sau

| Phép đo | Trước (HEAD `298b92c5`) | Sau (bản KT-238) |
|---|---|---|
| `cargo check -p vifast-evil` | RC=0 | RC=0 |
| lớp 1 thường | **XANH** `errors=0` | **XANH** `errors=0` (ngoài phạm vi lớp 1 — không đổi) |
| lớp 1 `--strict` | **XANH** `errors=0` | **XANH** `errors=0` |
| lớp 2 | **XANH** `errors=0 warnings=1` — chỉ `W_DEPINFO_SKIPPED` | **ĐỎ** `errors=1` — `E_DEPINFO_WAKE` tại `…/out/evil.rs:3` |
| phạm vi lớp 2 | (không có khái niệm) | **23 bảo vệ**, 1 từ chối rõ — `vifast-evil` **trong phạm vi** |
| tệp sinh đã quét | 0 | **1** |

Log: `kt238-logs/wb18-l1.log`, `wb18-l1strict.log`, `wb18-l2-before.log`, `wb18-l2-after.log`
(bản cuối của mã đã commit: `wb18-l2-after-final.log` — cùng kết quả ĐỎ).

**Lý do ĐỎ**: crate `vifast-evil` là thành viên workspace ⇒ **trong phạm vi bảo vệ**; tệp sinh
`$OUT_DIR/evil.rs` nằm trong target dir của `cargo metadata` ⇒ **được nạp + quét**; nội dung gọi
`m.wake(t)` ⇒ `E_DEPINFO_WAKE`. Không còn đường "thấy nhưng chỉ cảnh báo rồi PASS".

### 3.3 Đối chứng dương

| Ca | Kết quả |
|---|---|
| `v2/extra/ok` — cùng cơ chế `build.rs`→`$OUT_DIR`→`include!`, tệp sinh **lành** | lớp 2 **XANH** `errors=0`, **1 tệp sinh đã quét** (`kt238-logs/wb18-l2-positive.log`) |
| cây thật (22 crate bảo vệ, không tệp sinh) | lớp 2 **XANH** `errors=0` (`kt238-logs/final-l2.log`) |
| `v2/tools/evil-tool` — dưới `v2/tools/` **nhưng** phụ thuộc `vifast-sim` | **nâng** lên bảo vệ (24 bảo vệ / 1 từ chối) ⇒ tệp sinh bị bắt ⇒ **ĐỎ** (`kt238-logs/wb18-l2-promoted.log`) |

Đối chứng dương chứng minh bản vá **không** "đỏ toàn bộ tệp sinh": tệp sinh lành vẫn XANH; chỉ
nội dung vi phạm mới ĐỎ.

---

## 4. Các ca fail-open đã đóng (D2-188 chốt 2, 3)

| Ca | Trước | Sau | Ca selftest |
|---|---|---|---|
| `.d` rỗng/thiếu/không cho tệp nào | ĐỎ từ KT-227 | **ĐỎ** (giữ nguyên) | (23), (24), (25) |
| tệp rustc nạp bị **bỏ qua** | ngoài repo ⇒ ĐỎ; thư mục build / không phải tệp ⇒ **chỉ `W_`** | **mọi** trường hợp ⇒ ĐỎ: `E_DEPINFO_OUTSIDE` / `E_DEPINFO_SCOPE` / `E_DEPINFO_STALE`; **không còn `W_DEPINFO_SKIPPED`** | (30b), (46) |
| tệp trong `OUT_DIR`/`target` có token `wake` | **chỉ cảnh báo** | **ĐỎ** `E_DEPINFO_WAKE` | (30), (41) |
| tệp không đuôi `.rs` | đã quét từ KT-227 | **vẫn quét** (không đổi) | (26), (33) |
| tệp có dấu cách trong tên | đã xử lý escape từ KT-227 | **vẫn xử lý** (không đổi) | (4), (21) |
| thư mục tên `target` (`src/target/`) | đã quét từ KT-226 | **vẫn quét**; `target` **lạ** (không phải target dir metadata) ⇒ **từ chối rõ** | (28), (29), (30b) |
| tệp ngoài mọi crate | quét (không phân loại) | quét **+** `E_DEPINFO_SCOPE` | (44) |
| tệp sinh không quy được về crate | bỏ qua | `E_DEPINFO_SCOPE` | (43) |

**Fail-open còn lại trong phạm vi lớp 2: 0 đã biết.** Giới hạn **có chủ đích** giữ nguyên (không
phải fail-open mới): lớp 2 chỉ thấy nhánh `#[cfg]` **đang bật** và mã do `macro_rules!` sinh ra
không nằm trong tệp nào — lớp 1 phủ hai lớp ấy (đã ghi từ KT-217).

---

## 5. `OUT_DIR` gắn đúng lần build (D2-188 chốt 4)

Ba ràng buộc độc lập, mỗi cái đủ để loại `.d`/nguồn sinh cũ:

1. **Target dir từ `cargo metadata`**: `run_unit` đọc `.d` tại
   `<metadata.target_directory>/[<triple>/]debug/deps`, không tại hằng `target_rel`. `--fresh`
   truyền **cùng** `CARGO_TARGET_DIR` cho `cargo metadata` và `cargo check` ⇒ metadata trả đúng
   target dir tạm của lần build đang kiểm.
2. **Artifact/fingerprint của lần chạy**: chỉ `.d` có **đích** nằm trong tập `compiler-artifact`
   của chính lần `cargo check` này mới được dùng. Unit `fresh` vẫn được cargo phát
   `compiler-artifact` (fingerprint hợp lệ) ⇒ `.d` của nó là của lần build hiện tại. `.d` mồ côi
   (crate đã bỏ, hash đổi) **không** có artifact ⇒ **không đọc** — ca (45): `.d` cũ trỏ
   `…/out/evil.rs` cũ ⇒ `depinfo_used=0`, `loaded=∅`.
3. **Dấu lần build cho `--no-build`**: sau mỗi lần build xanh, ghi
   `<target_dir>/.kt238-depinfo-stamp.json` = `{version, unit, deps_dir, files: {tên .d: sha256}}`.
   `--no-build` **chỉ** đọc `.d` có trong dấu; thiếu dấu / `.d` đổi nội dung / `.d` trong dấu
   không còn trên đĩa ⇒ `E_DEPINFO_STALE` (fail-closed). Ca (47), (50), (51).

**Bằng chứng "tệp `OUT_DIR` cũ không bị đọc nhầm"**: ca (45) — `.d` cũ (artifact không thuộc lần
chạy) trỏ tệp sinh cũ chứa `wake` ⇒ **không** nạp, **không** lỗi; ca (47) — sửa nội dung `.d` sau
khi ghi dấu ⇒ `E_DEPINFO_STALE`; ca (51) — `--no-build` chỉ đọc `.d` trong dấu.

---

## 6. Không mở rộng mù quáng luật cấm `include!` không literal (D2-188 chốt 5)

- **Không** chạm `check-wake-paths.py`: phạm vi cấm `include!`/`#[path]` không literal của lớp 1
  **giữ nguyên** (mọi `include!` không phân giải tĩnh được trong phạm vi lớp 1 vẫn ĐỎ, kể cả tệp
  lành — báo oan đã ghi từ KT-233, **không** nới).
- Phần **mở rộng** của KT-238 chỉ ở lớp 2 và **phân biệt** rõ:
  - **mã sinh hợp lệ** = tệp nằm trong **target dir của `cargo metadata`**, được **rustc thật sự
    nạp** (có trong `.d` của lần build hiện tại), **quy được về crate bảo vệ** qua `package_id`
    của artifact, và **đã quét** R1/R2 ⇒ XANH nếu nội dung lành (ca (42));
  - **nguồn chưa kiểm soát** = tệp sinh **không** quy được về crate (ca (43)), tệp ngoài mọi
    crate (ca (44)), hoặc tệp dưới `target` **lạ** (ca (30b)) ⇒ `E_DEPINFO_SCOPE` — từ chối rõ.
- Tiêu chí ghi thành mã: `collect_depinfo` chỉ đưa vào `res.generated` khi `_under_any(p,
  target_dirs)`; `scan` báo `E_DEPINFO_SCOPE` khi `generated[p] is None` hoặc `classify(relp) is
  None`.

---

## 7. Kiểm bắt buộc

| Lệnh | Kết quả |
|---|---|
| `python3 v2/tools/check-wake-depinfo.py --selftest` | **52/52 ok** (37 cũ + 15 mới: (30b), (38)–(51)) |
| `python3 v2/tools/check-wake-paths.py --selftest` | **295/295 ok** (không mất ca) |
| `python3 v2/tools/check-wake-paths.py --root .` | `errors=0 warnings=0` |
| `python3 v2/tools/check-wake-paths.py --root . --strict` | `errors=0 warnings=0` |
| `python3 v2/tools/check-wake-depinfo.py --root .` | `errors=0 warnings=0` (22 bảo vệ / 1 từ chối rõ, 145 tệp, 0 tệp sinh) |
| `python3 v2/tools/check-wake-depinfo.py --root . --no-build` | `errors=0` (đọc đúng `.d` trong dấu) |
| `python3 v2/tools/check-wake-depinfo.py --root . --fresh` | `errors=0`, target dir = thư mục tạm từ metadata (`kt238-logs/final-l2-fresh.log`) |
| `./v2/bin/v2 check` | **PASS** (`kt238-logs/final-v2check.log`) |
| Repro WB-18 | **ĐỎ** `E_DEPINFO_WAKE` |
| Đối chứng dương | **XANH** |

---

## 8. Tệp thay đổi

- `v2/tools/check-wake-depinfo.py` — toàn bộ bản vá (không chạm `check-wake-paths.py`).
- `v2/agent/JOURNAL/kt238-metadata-scope.md` + `v2/agent/JOURNAL/kt238-logs/` — nhật ký + log.
- Repro (`v2/extra/evil`, `v2/extra/ok`, `v2/tools/evil-tool`, dòng `members` trong
  `v2/Cargo.toml`) **đã dọn sạch** sau khi đo; `git status` chỉ còn tệp công cụ + journal.

## 9. Rủi ro còn lại / bàn giao

- `cargo metadata` thêm ~0,3–0,5 s mỗi lần chạy (2 workspace + bao đóng); tổng thời gian lớp 2
  trên cây thật **12,65 s** (trước KT-238: 17,97 s khi nguội, 9,45 s khi ấm — chênh lệch chủ yếu
  do cache build, không do metadata).
- `--no-build` nay **bắt buộc** có dấu lần build; lần đầu trên máy mới sẽ ĐỎ cho tới khi chạy
  một lần build (đúng fail-closed, đã ghi trong `--help`/docstring).
- Không mở rộng R2 sang crate "từ chối rõ" — có chủ đích, kèm điều kiện chứng minh (bao đóng
  dependency không có crate bảo vệ). Nếu sau này `vifast-agentctl` thêm dep sản phẩm, nó **tự**
  được nâng lên bảo vệ (ca (39)).
