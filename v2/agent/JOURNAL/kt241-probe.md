# KT-241 — Harness compile-fail ranh giới API `NonReadyTarget` (luồng D)

**Ngày**: 2026-11-20. **Nhánh**: `kt241/D-probe` (base `c5a83552`). **Worktree**: `/private/tmp/kt241-D-probe`.
**Nguồn đặc tả**: [`kt241-spec/API-SPEC.md`](kt241-spec/API-SPEC.md) (D2-187/D2-194).
**Nguồn ca**: [`kt237-api-inventory.md`](kt237-api-inventory.md) §2.2/§2.3/§2.5/§3.3,
[`BYPASS-REGISTRY.md`](../BYPASS-REGISTRY.md) WB-16/WB-17.

---

## 0. TL;DR

| Việc | Kết quả |
|---|---|
| Tệp mới | `v2/tools/check-api-boundary.py` — **1.109 dòng**, chỉ stdlib, chạy bằng `python3` |
| Nối cổng | `v2/bin/v2` `do_check`: **1** bước `run_step "api boundary (KT-241)" …` (sau các bước wake) |
| Ca âm | **20 ca** × 2 crate (`vifast-core`, `vifast-sim`): WB-16 (6), WB-17 (7), primitive thô (3), chuyển đổi bị cấm (4) |
| Ca dương | **1 ca** × 2 crate: dựng đủ 10 biến thể `NonReadyTarget` + 5 cửa `Ready` ⇒ `cargo check` RC=0 |
| Stub selftest | **3/3 OK**: (a) stub đúng ⇒ XANH; (b) `transition_tcb` về `TcbState` ⇒ ĐỎ (thiếu 14 ca WB-16/17); (c) `write_state` `pub(crate)` ⇒ ĐỎ (thiếu 2 ca E0624) |
| Thời gian | stub **2,0 s**; mock API-có **3,1 s**; cây thật API-chưa-có **4,3 s** (≪ 90 s) |
| Khôi phục | `finally` + `atexit` + handler `SIGINT`/`SIGTERM`; SIGINT giữa lúc probe đã chèn: `rc=130`, `lib.rs` được ghi lại, `git status` sạch, 0 tệp probe sót |
| Nối cổng | `./v2/bin/v2 check` chạy **đúng** bước mới: `--- api boundary (KT-241): FAIL` (API chưa có) và `v2 check: 1 bước FAIL` — **0** bước khác hỏng |
| Trạng thái | Harness **ĐỎ trên cây thật hiện tại** vì A/B **chưa hợp nhất** API — đúng fail-closed, có gợi ý rõ trong log |

---

## 1. Thiết kế harness

### 1.1 Cơ chế

1. **Preflight**: `cargo check` crate đích **không** chèn probe ⇒ phải RC=0. Cây đỏ sẵn thì
   không đo được ranh giới (báo `cây KHÔNG xanh trước khi chèn probe`).
2. **Kiểm bẩn (fail-closed)**: `git status --porcelain -- <đúng các tệp sẽ chạm>`; khác rỗng ⇒
   **từ chối chạy** (exit 2), **không ghi đè**. Danh sách chạm của mỗi crate:
   `src/lib.rs` + `src/kt241_probe.rs` + `src/kt241_probe_helper.rs`.
   (Đặc tả nói "hai tệp"; WB-16 f2 "helper **tệp khác**" bắt buộc tệp thứ ba — harness kiểm
   **toàn bộ** tệp sẽ chạm, tức chặt hơn "hai tệp", không nới.)
3. **Chèn**: ghi tệp probe + nối dòng `mod kt241_probe;` / `mod kt241_probe_helper;` vào cuối
   `lib.rs` (giữ nguyên bản gốc trong bộ nhớ).
4. **Đo**: `cargo check --message-format=json` (JSON để lấy **mã lỗi** + `rendered` có
   "expected … found …"; `--message-format=short` làm mất phần expected/found).
5. **Khẳng định**: mỗi ca phải có chẩn đoán **đúng mã**, **đúng mẩu thông điệp**, **đúng
   `tệp:dòng`**. Dòng kỳ vọng tính từ marker `// CASE:<id>` ở cuối dòng trong chính nguồn probe
   ⇒ **không có số dòng viết tay**, không lệch khi sửa tệp. Chẩn đoán **lạ** (sai mã / sai dòng /
   ngoài tệp probe) ⇒ ĐỎ.
6. **Ca dương**: chèn tệp probe **khác** dùng đúng API ⇒ `cargo check` phải RC=0 và 0 lỗi.
   Ca dương đỏ ⇒ harness ĐỎ (không được "chặn bằng cách làm hỏng biên dịch").
7. **Khôi phục**: `finally` (mọi đường lỗi) + `atexit` + handler `SIGINT`/`SIGTERM`
   (`sys.exit(128+signum)` ⇒ `finally` chạy). Sau khôi phục: kiểm lại `git status` sạch cho
   đúng các tệp đó; **không sạch ⇒ ĐỎ** (`RestoreError`, exit 1).

### 1.2 Lệnh cargo

| Crate | Lệnh | Ghi chú |
|---|---|---|
| `vifast-core` | `cargo check --manifest-path v2/kernel/Cargo.toml -p vifast-core --message-format=json --color=never` | workspace kernel tách riêng (`v2/kernel/Cargo.toml`), host target |
| `vifast-sim` | `cargo check --manifest-path v2/Cargo.toml -p vifast-sim --message-format=json --color=never` | workspace host |

Toolchain ghim theo `v2/rust-toolchain.toml` (đọc `channel`, đặt `RUSTUP_TOOLCHAIN` nếu chưa có)
— cùng cách `v2/bin/v2` làm, để harness chạy tay không lệch toolchain cổng.

### 1.3 Chế độ

| Lệnh | Việc |
|---|---|
| `python3 v2/tools/check-api-boundary.py --root <REPO>` | chạy trên **cây thật** (core + sim), ca âm + ca dương |
| `python3 v2/tools/check-api-boundary.py --stub-selftest` | dựng crate stub trong `/tmp`, chạy **3 lượt** (a)(b)(c) — đối chứng âm của harness |
| `--verbose` / `--keep` / `--timeout N` | in mọi chẩn đoán / giữ stub / timeout mỗi cargo (mặc định 300 s) |

Exit: `0` xanh, `1` đỏ, `2` dùng sai / **từ chối** (tệp đã bẩn).

> **Vì sao cờ tên `--stub-selftest`** (không phải `--selftest`): `check-selftest-wiring.py`
> (KT-134) coi mọi script có cờ `--selftest` là "script tự kiểm" và đòi **dòng cổng có cờ đó**.
> Đặc tả KT-241 chỉ cho thêm **một** dòng cổng **không** cờ ⇒ nếu đặt tên `--selftest`, harness
> thành `E_ORPHAN` và `v2 check` đỏ ở bước wiring. Tên `--stub-selftest` không khớp token
> `--selftest` nên không vào lớp đó; đã đo: `check-selftest-wiring: PASS (16 script, 0 mồ côi)`.

---

## 2. Bảng ca

Cột "dòng" = dòng trong tệp probe do harness sinh (tính từ marker, khớp log cây thật bên dưới).
Cột "đo được" = mã + thông điệp **đo thật** trên stub (`/tmp/kt241-measure`, cùng nguồn probe).

| # | Dạng tấn công | Mã kỳ vọng | Thông điệp đo được | Dòng |
|---|---|---|---|---|
| WB-16 f1 | helper **cùng tệp** `fn helper(…, s: TcbState)` | `E0308` | `expected `NonReadyTarget`, found `TcbState`` | probe:8 |
| WB-16 f2 | helper **tệp khác** (`kt241_probe_helper.rs`) | `E0308` | như f1 | helper:4 |
| WB-16 f3 | hàm trả về `TcbState` (`fn ready_state()`) | `E0308` | như f1 | probe:19 |
| WB-16 f4 | `const R: TcbState = TcbState::Ready` | `E0308` | như f1 | probe:25 |
| WB-16 f5 | `static S: TcbState = TcbState::Ready` | `E0308` | như f1 | probe:31 |
| WB-16 f6 | closure `\|s: TcbState\| m.transition_tcb(t, s)` | `E0308` | như f1 | probe:35 |
| WB-17 f1 | `let tt = Machine::transition_tcb; tt(m,t,Ready)` | `E0308` | như f1 | probe:45 |
| WB-17 f2 | fn-pointer **có kiểu** `fn(…, TcbState)` | `E0308` | `expected fn pointer, found fn item` | probe:49 |
| WB-17 f3 | truyền qua tham số `fn(…, TcbState)` | `E0308` | `expected fn pointer, found fn item` | probe:61 |
| WB-17 f5a | generic `F: Fn(…, TcbState)` | `E0631` | `type mismatch in function arguments` | probe:73 |
| WB-17 f5b | trait object `&dyn Fn(…, TcbState)` | `E0631` | `type mismatch in function arguments` | probe:77 |
| WB-17 f6 | UFCS `Machine::transition_tcb(m,t,Ready)` | `E0308` | `expected `NonReadyTarget`, found `TcbState`` | probe:81 |
| WB-17 f7 | `<Machine>::transition_tcb(m,t,Ready)` | `E0308` | như f6 | probe:85 |
| raw-direct | module **anh em** gọi `m.write_state(t, Ready)` | `E0624` | `method `write_state` is private` | probe:93 |
| raw-wrapper | `pub fn evil(m,t) { m.write_state(t, Ready) }` | `E0624` | `method `write_state` is private` | probe:97 |
| typed-wrapper | wrapper nhận `TcbState` rồi gọi `transition_tcb` | `E0308` | `expected `NonReadyTarget`, found `TcbState`` | probe:101 |
| conv-from | `NonReadyTarget::from(TcbState::Ready)` | `E0308`/`E0599`/`E0277` | `expected `NonReadyTarget`, found `TcbState`` | probe:109 |
| conv-into | `let _: NonReadyTarget = TcbState::Ready.into()` | `E0277`/`E0599` | `the trait bound `NonReadyTarget: From<TcbState>` is not satisfied` | probe:113 |
| conv-tryinto | `TcbState::Ready.try_into()` | `E0277`/`E0599` | `the trait bound `NonReadyTarget: TryFrom<TcbState>` is not satisfied` | probe:117 |
| conv-into-state | `target.into_state()` từ ngoài module | `E0624`/`E0599` | `method `into_state` is private` | probe:122 |

**Ghi chú `conv-from`**: đo thật cho `E0308`, **không** phải `E0599`. Lý do: `NonReadyTarget::from`
phân giải về impl **phản xạ** `impl<T> From<T> for T` của core, nên lỗi là *sai kiểu đối số*
("expected `NonReadyTarget`, found `TcbState`") — vẫn **đúng nguyên nhân** "không có chuyển đổi
từ `TcbState`". Nhóm mã của ca này vì vậy gồm `E0308`/`E0599`/`E0277` (đặc tả cho phép nhóm cho
đường chuyển đổi), nhưng **thông điệp** vẫn bị khẳng định chặt. Ca `conv-into` bổ sung dạng
`E0277` chuẩn của "không có `From`".

**Ca dương** (mỗi crate): `transition_tcb(t, NonReadyTarget::Inactive)`,
`NonReadyTarget::BlockedOnSend { ep, call, fault }`, `BlockedOnRecv`, `BlockedOnReply`,
`BlockedOnNotification`, `BlockedOnAddress`, `Sleeping`, `Running`, `Stopping`, `Dead`,
`start_tcb(t)`, `tcb_yield(t)`, `block_current(0, NonReadyTarget::Sleeping { reason })`
⇒ `cargo check` **RC=0**. Dựng đủ 10 biến thể để chốt luôn hình dạng enum (không
`#[non_exhaustive]`, đúng tên trường).

---

## 3. Ba lần chạy stub (đối chứng âm của harness)

Crate stub dựng trong `/tmp` (không nằm trong repo), mô phỏng đúng API mới: `NonReadyTarget` 10
biến thể, `transition_tcb(to: NonReadyTarget)`, `write_state` **private** trong module
`state_write`, `into_state` private. Log đầy đủ:
[`kt241-logs/stub-selftest.log`](kt241-logs/stub-selftest.log).

```text
== KT-241 api boundary — STUB SELFTEST (/tmp/kt241-api-probe-z1w79_7w) ==
--- (a) stub ĐÚNG: transition_tcb(to: NonReadyTarget), write_state private
    RC ca âm=101, RC ca dương=0, verdict=XANH (kỳ vọng XANH) => OK
--- (b) ĐỔI KIỂU NGƯỢC: transition_tcb(to: TcbState)
    RC=101, verdict=ĐỎ (kỳ vọng ĐỎ), thiếu 14 ca WB-16/17 => OK
--- (c) NỚI QUYỀN: write_state -> pub(crate)
    RC=101, verdict=ĐỎ (kỳ vọng ĐỎ), thiếu 2 ca E0624 => OK
== STUB SELFTEST: OK — assertion có răng (3/3) (2.0 s) ==
```

| Lượt | Đột biến | RC | Verdict | Kỳ vọng | Kết luận |
|---|---|---|---|---|---|
| (a) | không (stub đúng) | âm 101 / dương 0 | **XANH** | XANH | OK — 20/20 ca âm đúng mã+thông điệp+dòng, ca dương biên dịch |
| (b) | `transition_tcb(to: TcbState)` | 101 | **ĐỎ** | ĐỎ | OK — **thiếu 14 ca** WB-16/17 (ca hết đỏ ⇒ harness bắt được) |
| (c) | `write_state` → `pub(crate)` | 101 | **ĐỎ** | ĐỎ | OK — **thiếu 2 ca** `raw-direct`/`raw-wrapper` (E0624 biến mất ⇒ harness bắt được) |

Nghĩa là assertion **có răng**: nếu ai đó nới API (đổi kiểu ngược) hoặc nới quyền primitive thô,
harness **không** còn xanh.

---

## 4. Khôi phục + bằng chứng `git status` sạch

### 4.1 Cây thật (API chưa có — ĐỎ đúng, có gợi ý)

Log: [`kt241-logs/real-tree-before-merge.log`](kt241-logs/real-tree-before-merge.log).
Sau khi chạy:

```text
$ git status --porcelain
 M v2/bin/v2
?? v2/agent/JOURNAL/kt241-logs/
?? v2/tools/check-api-boundary.py
```

Không có `v2/kernel/vifast-core/src/kt241*`, không có `v2/sim/vifast-sim/src/kt241*`, `lib.rs`
hai crate **không** bị sửa. Harness in đúng gợi ý:

```text
GỢI Ý: API KT-241 CHƯA CÓ trong cây (A/B chưa hợp nhất) — thiếu `NonReadyTarget`;
đây là ĐỎ đúng (ranh giới chưa tồn tại).
```

### 4.2 SIGINT giữa lúc probe **đã** chèn

Log: [`kt241-logs/sigint-restore.log`](kt241-logs/sigint-restore.log). Test trên mock repo:
poll tới khi thấy `kt241_probe.rs` xuất hiện rồi gửi `SIGINT`:

```text
probe_seen=True rc=130 lib_mtime_changed=True dirty='' leftover=[]
```

⇒ trap chạy: `lib.rs` được ghi lại (mtime đổi), `git status` sạch, **0** tệp probe sót.

### 4.3 Fail-closed khi tệp đã bẩn

Log: [`kt241-logs/fail-closed-dirty.log`](kt241-logs/fail-closed-dirty.log). Cố tình thêm marker
vào `lib.rs` của mock rồi chạy:

```text
check-api-boundary: TỪ CHỐI — tệp sẽ chạm đã bẩn — từ chối chạy (fail-closed), không ghi đè:
M v2/kernel/vifast-core/src/lib.rs
RC=2
```

Marker còn nguyên trong tệp (không ghi đè), không có tệp probe nào được tạo.

### 4.4 Chế độ cây thật khi API **có** (mock repo)

Vì A/B chưa hợp nhất, dựng **mock repo** `/tmp/kt241-mock` mô phỏng đúng bố cục thật
(`v2/kernel/Cargo.toml` + `v2/Cargo.toml`, `vifast-core` `no_std`, `vifast-sim` với
`kobj/state_write.rs`) và **có** API mới. Log:
[`kt241-logs/mock-api-present.log`](kt241-logs/mock-api-present.log):

```text
--- vifast-core: preflight ... OK (0.2 s)
--- vifast-core: ca âm (20 ca) ... OK (0.3 s)
--- vifast-core: ca dương (API dùng được) ... OK (0.2 s)
--- vifast-sim: preflight ... OK (0.9 s)
--- vifast-sim: ca âm (20 ca) ... OK (0.3 s)
--- vifast-sim: ca dương (API dùng được) ... OK (0.2 s)
== KT-241 api boundary: OK (3.1 s) ==
```

⇒ Cơ chế chèn/khôi phục + khẳng định chạy đúng trên **cả hai** crate khi API tồn tại; mock sạch
sau khi chạy (`git status` rỗng).

### 4.5 Nối cổng: `./v2/bin/v2 check` chạy đúng bước mới

Chạy **toàn bộ** `./v2/bin/v2 check` trên cây thật (API chưa có). Kết quả: **1 bước FAIL — đúng
bước `api boundary (KT-241)`**, mọi bước khác OK (fmt, clippy, rules, K6, các selftest, wake
paths/dep-info, test-teeth, abi, golden, scn, report, gen-check, build). Trích
[`kt241-logs/v2-check-api-step.log`](kt241-logs/v2-check-api-step.log):

```text
--- api boundary (KT-241)
== KT-241 api boundary (compile-fail) — /private/tmp/kt241-D-probe ==
...
== KT-241 api boundary: ĐỎ (3.5 s) ==
--- api boundary (KT-241): FAIL
...
--- build (L0 compile): OK
v2 check: 1 bước FAIL
```

⇒ Dòng nối cổng **chạy được**, thời gian trong ngữ cảnh cổng **3,5 s**, và **không** làm hỏng
bước nào khác. Sau khi A/B hợp nhất, bước này phải chuyển thành `OK` (xem §6.2).

---

## 5. Thời gian đo

| Bối cảnh | Thời gian | Ghi chú |
|---|---|---|
| Stub selftest (3 lượt, 6 cargo check) | **2,0 s** (tổng tiến trình 2,2 s) | crate tí hon |
| Mock repo, API có (2 crate × 3 cargo check) | **3,1 s** | |
| Cây thật, API chưa có (2 crate × 3 cargo check) | **4,3 s** | preflight dùng cache |
| Cây thật, trong `./v2/bin/v2 check` | **3,5 s** | sau clippy, cache ấm |
| Ước lượng sau hợp nhất | **~8–12 s** | core ~1,4 s/lần, sim ~2–3 s/lần (đo cold: 2,0 s / 3,5 s) |

≪ ngưỡng ~90 s ⇒ **giữ ở `check`** là hợp lý; không cần chuyển sang `gate`.

---

## 6. Điều **chưa** kiểm được (vì API thật chưa có) + lệnh lead chạy sau hợp nhất

### 6.1 Chưa kiểm được

1. **Ca âm trên cây thật**: hiện `NonReadyTarget` chưa tồn tại ⇒ probe chết ở `E0432`
   (`unresolved import`) trước khi đo được WB-16/17. Đã kiểm **cùng nguồn probe** trên stub
   (mục 3) và trên mock đúng bố cục (mục 4.4), nhưng **chưa** trên `v2/kernel/vifast-core` +
   `v2/sim/vifast-sim` thật.
2. **Ca dương trên cây thật**: `start_tcb`/`tcb_yield` chưa có ⇒ chưa xác nhận được chữ ký thật
   khớp đặc tả (đã xác nhận trên stub/mock).
3. **Vị trí `write_state` private thật**: chưa xác nhận `E0624` (hiện là `E0599` vì method chưa
   tồn tại) — sau hợp nhất phải là `E0624` đúng thông điệp.
4. **`NonReadyTarget` re-export** ở `vifast_core` gốc và `vifast_sim::kobj`: chưa xác nhận.

### 6.2 Lệnh lead chạy **sau khi A/B hợp nhất** (theo thứ tự)

```bash
cd /private/tmp/kt241-D-probe        # hoặc worktree hợp nhất

# 1) Harness trên cây thật — kỳ vọng: "== KT-241 api boundary: OK (… s) ==", RC=0.
python3 v2/tools/check-api-boundary.py --root "$PWD"

# 2) Đối chứng âm của harness (stub /tmp) — kỳ vọng: "STUB SELFTEST: OK — assertion có răng (3/3)".
python3 v2/tools/check-api-boundary.py --stub-selftest

# 3) Bước cổng đã nối — kỳ vọng dòng "--- api boundary (KT-241): OK".
./v2/bin/v2 check

# 4) Bằng chứng không để lại tệp probe:
git status --porcelain -- v2/kernel/vifast-core/src v2/sim/vifast-sim/src
#    kỳ vọng: rỗng (không có kt241_probe*.rs, lib.rs không đổi)
```

Nếu bước 1 ĐỎ: đọc mục `THIẾU`/`LẠ` — mỗi dòng ghi `ca | tệp:dòng kỳ vọng | mã+thông điệp kỳ
vọng`; `LẠ` là chẩn đoán sai nguyên nhân (thường do A/B lệch đặc tả: thiếu re-export, sai tên
trường, `write_state` còn `pub(crate)`, hoặc `transition_tcb` chưa đổi kiểu).

---

## 7. Ghi chú triển khai

1. **Ba tệp chạm mỗi crate** (không phải hai): `src/lib.rs` + `src/kt241_probe.rs` +
   `src/kt241_probe_helper.rs`. WB-16 f2 ("helper **tệp khác**") bắt buộc tệp thứ ba; harness
   kiểm bẩn **toàn bộ** danh sách này (chặt hơn, không nới).
2. **Không sửa mã sản phẩm**: harness chỉ *chèn tạm* rồi khôi phục; `git status` sau chạy chỉ
   còn tệp mới của luồng D (`v2/tools/check-api-boundary.py`, journal, log).
3. **Không `#[allow]`** trong probe: cảnh báo (dead_code/unused) không làm `cargo check` đỏ, nên
   không cần và không dùng.
4. **Không commit** (lead commit). Không `git checkout`/`stash`/`reset` trên cây thật.
5. **`--stub-selftest` không nằm trong cổng**: đặc tả chỉ cho **một** dòng cổng (không cờ); stub
   selftest là kiểm một lần + chạy tay lại được. Nếu lead muốn nó chạy trong cổng, thêm **dòng
   thứ hai** `run_step "selftest api boundary (KT-241)" "$PY" … --stub-selftest` — khi đó có thể
   đổi tên cờ thành `--selftest` cho đúng quy ước (wiring checker sẽ thấy dòng cổng có cờ).
6. **Log**: `v2/agent/JOURNAL/kt241-logs/` — `stub-selftest.log`, `real-tree-before-merge.log`,
   `mock-api-present.log`, `fail-closed-dirty.log`, `sigint-restore.log`.
