<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R5-viscript — báo cáo vòng 5 (chốt VS1c), dòng ViScript

Agent 2 · 2026-11-13 · `head` = `58080a9` là **head lúc thu số liệu** (QUEUE có đủ **49 hàng** 6 cột;
ST-32..ST-35 `DONE`, ST-36 `READY`); commit chứa báo cáo là commit `[ST-36]` kế tiếp. Số đo chạy trên
**mã của `a0b5525`** (hai commit `a0b5525..58080a9` chỉ chạm `QUEUE`/`DECISIONS`/`STATE` của Agent 0).
Bản máy đọc: [`R5-viscript.json`](R5-viscript.json) — **đúng 11 khoá** Khối E (`project = "viscript"`,
`round = 5`).

## 1. Tóm tắt (5 dòng)

1. **VS1c chạy được đầu–cuối**: `json.decode` (round-trip số/bool/`none`/chuỗi/list/map/bản ghi, escape đủ +
   **surrogate pair**, lỗi kèm **byte offset**), **`std.fs` tối thiểu qua `Host`** (`FsHost`; không host ⇒
   `VS0604` khoá **cả hai đường**), **`| complete`** + **pipe→stdin** cho chương trình ngoài, **generic THẬT**
   (`Hop[int] ≠ Hop[str]`; `Or[int, str]`; hàm hợp nhất **liên kết**).
2. **Hai đường trùng ngữ nghĩa**: differential **100.000 chương trình / 0 lệch** (so **đủ** `(mã, dòng, cột,
   thông điệp)`), parity **320/320** trên **346** ca sem ⇒ **0 lệch**; payload generic vẫn **xoá kiểu** ⇒
   `codegen`/`VBC1` **không đổi**.
3. **507 test workspace** (21 diag + 96 syntax + 18 fmt + **143** types + 24 interp + 50 bytecode + 9 vm +
   **146** cli); **346 ca ngữ nghĩa / 18 nhóm** (327 `.vis` + 19 `.vish`, 52 `.err`).
4. **`vis test`**: TAP `1..346` — **346 ok**; `vis test --vm` = **346 ok / 6 SKIP / 0 đỏ** (SKIP = ca cần
   **host chương trình ngoài**, VM không có host — ST-27). **26 ca lỗi biên dịch ĐÃ so** chẩn đoán
   (`compile()` không panic, 0 lệch) — đóng nợ R4 §5.6.
5. **`./vis-dev gate --fast` PASS** + **`./vis-dev nightly --fuzz-minutes 1` PASS** (fuzz parser **3.035.575
   ca / 0 vi phạm**, 60,0 s); khởi động ấm **11,2–11,7 ms** (debug build) — **đạt** §19 `< 20 ms [ƯL]`.

## 2. Bảng task vòng 5 (ST-32 → ST-36)

| ID | Trạng thái | Commit | Cổng | Kết quả đo được |
|---|---|---|---|---|
| ST-32 | DONE | `774369b` | fast | **harness chẩn đoán 3 tầng**: `compile()` **không panic** trên HIR lỗi (`CODEGEN_TIN_TYCK = 21`); 26 ca lỗi biên dịch **được chạm** đường bytecode; **443 test** lúc đóng |
| ST-33 | DONE | `f9e6db5` | fast | **`json.decode`** (`Result[Any, str]`, object giữ thứ tự, trùng khoá ⇒ giá trị cuối/vị trí đầu, tràn i64 ⇒ `Err` ở **byte đầu token**, surrogate pair, depth 128) + **`std.fs`** (`read/write/exists/list`, `list` sắp xếp **không đệ quy`) qua **`FsHost`**; **`VS0604`** khoá **cả interp lẫn VM**; `--explain-perms` in `FS …`; **466 test** |
| ST-34 | DONE | `4ababc6` | fast | **`| complete`** ⇒ record `{stdout, stderr, code}`, exit≠0 **hết** `VS0501`; **pipe→stdin** (`list[str]` nối `\n` + newline cuối, `str` nguyên văn); `MAX_STDIN` **8 MiB** (báo số byte thật, **không spawn**), thread ghi + `drop`, timeout **nêu stage**; **482 test** |
| ST-35 | DONE | `a0b5525` | fast | **generic THẬT**: `Hop[int]`/`Hop[str]` là **hai** `RecId` (interned theo tổ hợp), trường thay `T`, `Or[int, str]` riêng, hàm `f[T](x: T, h: Hop[T])` **hợp nhất liên kết**; payload **xoá kiểu** ⇒ `codegen`/`VBC1` không đổi; sem **338 → 346**; **507 test** |
| ST-36 | READY → **báo cáo này** | — | fast | tệp này + `R5-viscript.json` (QUEUE@head ghi `READY`) |

**49 hàng QUEUE** khớp đúng `queue_at(58080a9)`; 8 hàng `BLOCKED` (ST-37..ST-43 dòng VS2) có mặt trong
`blocked[]`.

## 3. Số đo (mọi số tái lập bằng lệnh ghi kèm; chạy 2026-11-13, giờ UTC ghi trong ngoặc)

| Số | Giá trị | Lệnh |
|---|---|---|
| test workspace | **507** = 21 diag + 96 syntax + 18 fmt + 143 types + 24 interp + 50 bytecode + 9 vm + 146 cli | `cargo test --workspace`; theo crate `cargo test -p <crate>` (07:5xZ) |
| ca ngữ nghĩa | **346 ca / 18 nhóm** (19 thư mục, `lệnh-vish` rỗng) | `find test/sem -name '*.vis' -o -name '*.vish' \| wc -l`; `ls -d test/sem/*/` |
| ca ngữ nghĩa theo đuôi | **327 `.vis` + 19 `.vish`**; **52 `.err`** | `find test/sem -name '*.vis' \| wc -l`, `… -name '*.vish'`, `… -name '*.err'` |
| `vis test` (interp) | **346 ok / 0 `not ok`**, exit 0; `1..346` | `vis test` |
| `vis test --vm` | **346 ok** (**6 SKIP** = chương trình ngoài cần host) **/ 0 đỏ**, exit 0 | `vis test --vm` |
| parity chẩn đoán | **320/320 khớp, 0 lệch** (so **đủ** `(mã, dòng, cột, thông điệp)`; **26** ca lỗi **biên dịch** đã so qua `compile()` — 0 lệch) | `cargo test -p vis-vm --test vs_interp` |
| differential sinh tự động | `DIFF tong=100000 loi_tinh=0 chay=100000 lech=0 giay=200.4 toc_do=499/s seed=3523158054` — so **đủ** `(mã, dòng, cột, thông điệp)` (07:5xZ) | `./bin/vis-diff` (100.000; `VIS_DIFF_N` để đổi) |
| fuzz parser | **3.035.575 ca / 68.968.426 token / 0 vi phạm** (60,0 s, 50.593 ca/s) — chạy **mới** trong `nightly --fuzz-minutes 1` (07:5xZ) | `./vis-dev nightly --fuzz-minutes 1` |
| fuzz đột biến verifier | bản ngắn trong cổng: `cargo test -p vis-vm --test fuzz_mutate` (**294 module**, 1,4 s); số **10 phút** của ST-21 là **số cũ**, không chạy lại | `cargo test -p vis-vm --test fuzz_mutate` |
| VBC1 | **53 tag** (`Instr`, version 1), hash nguồn **32 byte = SHA-256** — **không đổi** suốt vòng 5 | `Instr` trong `crates/vis-bytecode/src/format.rs`; `crates/vis-bytecode/tests/hash.rs` |
| sổ mã | **7 sổ**, `all_codes()` **28** (10 + 2 + 2 + 4 + 4 + 3 + 3) | `crates/vis-diag/src/lib.rs` (`BOOKS`); `crates/vis-diag/tests/messages.rs` |
| khởi động ấm | **11,73 ms** (interp) · **11,62 ms** (`--vm` cache) · **11,16 ms** (`--vm --no-cache`) — trung vị 15 mẫu, debug build | `vis run [--vm] <tệp>` lặp 16 lần, đo bằng `perf_counter` |
| cổng | **PASS** | `./vis-dev gate --fast` (07:4xZ, và chạy lại sau khi ghi báo cáo) |
| nightly | **PASS** | `./vis-dev nightly --fuzz-minutes 1` (07:52Z) |

**Số không tái lập được** (không ghi vào báo cáo): số token golden, số snapshot `render`, số ca tyck từng tệp —
chỉ tồn tại trong kỳ vọng của harness.

## 4. Quyết định vòng 5 (VD-64..VD-68, mỗi cái một dòng)

- **VD-64** — **VS1c mở**: `json.decode` **có** (round-trip; `null ⇒ none`; nợ `Ty::Any`); **`std.fs` tối
  thiểu qua `Host`** (`FsHost`, không host ⇒ `VS0604`); `| complete`/pipe→stdin là **VS1c**; generic **thật**
  theo tổ hợp là **VS1c**.
- **VD-65** — **harness chẩn đoán 3 tầng**: `CODEGEN_TIN_TYCK = 21` (codegen **tin** tyck, `compile()` không
  panic trên HIR lỗi); 26 ca lỗi biên dịch **được chạm** đường bytecode ⇒ **đóng nợ R4 §5.6**.
- **VD-66** — **`std.fs`** qua trait `FsHost` trên `Host` (mặc định `None ⇒ VS0604`); `vis-cli` cài bằng
  `std::fs`; **`--explain-perms`** in `FS read|write|exists|list`; cho phép đường dẫn tuyệt đối/`..`
  (**consent thật + `cap[Dir]` ⇒ VS2**); `list` sắp xếp, không đệ quy.
- **VD-67** — **pipe là GIÁ TRỊ**, không phải stream: stdin = **đối số vị trí thứ 2** của `exec.<ten>`
  (không đổi HIR/codegen); `| complete` **đổi tên callee** `exec.[complete.][raw.]<ten>` ⇒ record
  `{stdout, stderr, code}`; `MAX_STDIN` 8 MiB, timeout 10 s **/stage** (không cap tổng).
- **VD-68** — **generic thật**: mỗi tổ hợp tham số kiểu là **một** `RecId`/`EnumId` (instantiation interned,
  `inst_args`/`rec_pos`); `Hop[int] ≠ Hop[str]`; `T` chưa ràng buộc ⇒ **giữ** xoá kiểu; payload vẫn xoá
  kiểu ⇒ **không** đổi `codegen`/`VBC1`.

## 5. Lỗ hổng đã biết (đầy đủ đầu vòng 6 — gồm cái **đóng** và cái **mới**)

**Đã đóng / đã nới trong vòng 5**

1. **26 ca lỗi biên dịch chưa so chẩn đoán hai đường** — **ĐÃ so** (ST-32): `compile()` không panic, 0 lệch.
2. **`grep` không khớp (`exit 1`) bị coi là lỗi** — **HẾT** (ST-34): `| complete` ⇒ `code = 1`, không `VS0501`.
3. **`json.decode` chưa có** — **CÓ** (ST-33) round-trip + escape đủ + surrogate pair + byte offset.
4. **`std.fs` chưa có** — **CÓ** (ST-33) qua `FsHost` (không host ⇒ `VS0604` **cả hai đường**).
5. **`Hop[int] ≡ Hop[str]`** — **HẾT** (ST-35): hai `RecId` riêng, trường thay `T`, enum `Or[int, str]` riêng.

**Còn lại (có chủ ý hoặc dồn VS2)**

6. **`codegen tin tyck`** — `CODEGEN_TIN_TYCK = 21` (VD-65): codegen không kiểm lại; front end là cổng.
7. **`Ty::Any` cho `json.decode`/`| complete`** — chưa kiểm tĩnh (`let v: int? = json.decode(…)` ⇒ `VS0412`),
   đóng khi có `Any`/generic thật ở `Type` (VS2).
8. **Pipeline chưa là BIỂU THỨC ở `.vish`** — chỉ mức câu lệnh ⇒ `let r = a | b | complete; r.code` chưa dùng được.
9. **`&self`/`&mut self` chưa có** — `trait`/`impl` dùng `self` **theo giá trị** (VS2 §3.7).
10. **Dispatch chỉ khi kiểu TĨNH** — `x.m()` với `x` generic/`Any`/kết quả `std.list` ⇒ `VS0403`.
11. **Mồ côi (orphan) mới kiểm trong MỘT tệp**; **siêu trait** (`trait A: B`) chưa kiểm.
12. **`derives` chưa SINH MÃ** — `Eq`/`Show`/`Json` dùng ngữ nghĩa có sẵn.
13. **`Hop[Hop[int]]` chưa có id cho lớp trong** — chạy được (payload xoá kiểu) nhưng không đơn hình hoá lớp trong.
14. **Consent thật/`cap[Proc]`/`cap[Dir]` ⇒ VS2** — hiện allowlist 8 + timeout + ghi `FS …` khi `--explain-perms`.
15. **Không cap TỔNG timeout** — chuỗi N stage ngoài có thể tới **N×10 s** (VD-67, chấp nhận có ý thức).
16. **`--desc`** (mô tả gói/cờ) chưa có; **`pkg.*`/registry** ⇒ **VS2**.
17. **Enum/`Result` chưa round-trip JSON** (`{"ok":…}`/`{"variant":…}`) — nợ ST-33.
18. **`nfree` của closure = cả khung** (thừa, ST-22 chưa tối ưu); **`steps` khác đơn vị** interp ↔ VM.
19. **`match`/`if` làm đối số** chưa parse; **nội suy không đệ quy**; `\u{…}` chưa hỗ trợ; **`char` escape**
    `VS0105` (theo VD-30); **`None(1)`** chưa kiểm đối số; **map literal trùng khoá** giữ cả hai;
    **closure snapshot** (VD-51).
20. **`vis fmt` cho `.vish`** chưa có đặc tả ⇒ `Bail out!`; cache lợi ích nhỏ, chi phối là khởi động tiến trình.

## 6. Đối chiếu điều kiện chốt VS1c (§19) + đề xuất

| Điều kiện §19 (VS1c) | Kết quả | Bằng chứng |
|---|---|---|
| differential interp ↔ VM trên **100.000** chương trình, 0 lệch | ✓ **100.000 chương trình / 0 lệch**, so **đủ** `(mã, dòng, cột, thông điệp)` | `./bin/vis-diff` |
| khởi động ấm **< 20 ms [ƯL]** | ✓ **11,16–11,73 ms** trung vị (debug build) | phép đo 15 mẫu, §3 |
| bytecode từ cache **luôn** qua verifier | ✓ `load_verified` trên mọi đường nạp | `crates/vis-bytecode/src/verify.rs` |
| fuzz bytecode/parser không panic/vô hạn | ✓ parser **3.035.575 ca / 0 vi phạm** (60 s); verifier bản ngắn trong cổng | `./vis-dev nightly --fuzz-minutes 1`; `fuzz_mutate` |
| `json.decode` + `std.fs` + `| complete` + pipe→stdin | ✓ **có** (ST-33/ST-34) | §2; `crates/vis-cli/tests/std_fs_json.rs`, `pipeline.rs` |
| generic **thật** (`Hop[int] ≠ Hop[str]`) | ✓ **có** (ST-35); lớp trong lồng còn nợ | `crates/vis-types/tests/generic_that.rs` |
| `vis run/check/fmt/test` đầy đủ | ✓ `vis test` **346 ok**, `--vm` **346 ok / 6 SKIP** | `vis test`; `gate --fast` |
| **Chưa đạt — dồn VS2**: `async`/`await`, kênh, net, quyền Landlock thật (`consent`/`cap[Proc]`/`cap[Dir]`), LSP, gói/registry (`pkg.*`), `vis-host-windows`, **wasm 4 nền tảng** | **VS2** | §5 |

**Đề xuất cho Agent 0** (tôi **không** sửa các tệp này):

1. **`AGENTS.md` §8**: cập nhật **vòng 5** đã chốt (`ST-32..ST-36`, `R5-viscript`, **507** test / **346** ca /
   parity **320/320** / sem tăng **có ý thức** 338→346) và ghi **VS1c DONE**; nêu `| complete`, pipe→stdin,
   `json.decode`, `std.fs`, generic thật (VD-64..VD-68).
2. **`viscript/AGENTS.md` §3/§5**: dải quyết định **`VD-01..VD-68`**; thêm vào mục lệnh: `VIS_CMD_TIMEOUT_MS`,
   `VIS_DIFF_N`, `--explain-perms`; ghi rõ **`Ty::Any`** là nợ có chủ ý.
3. **Hàng QUEUE thiếu cột**: `ST-20b` trong QUEUE **hiện đã đủ 6 cột** (R4 từng nêu); nếu còn hàng 5 cột thì
   `report-check` sẽ bỏ qua — nên kiểm `awk -F'|' 'NF<8' viscript/agent/QUEUE.md` trước mỗi vòng.
4. **`docs/`** (chỉ Agent 0/1): §19 nên ghi "khởi động ấm" đo trên **đường `--vm`** (cache) và ghi **`Hop[Hop[int]]`**
   là giới hạn còn lại của VS1c (VD-68); ghi **`Ty::Any`** cho `json.decode`/`complete`.
5. **Nợ ưu tiên vòng 6**: `Ty::Any`/`Any` thật (đóng `let v: int? = json.decode(…)`), pipeline **mức biểu
   thức** ở `.vish`, `&self`, enum/`Result` round-trip JSON, cap **tổng** timeout (nếu §9.4 đòi), `--desc`.
