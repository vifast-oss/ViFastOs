<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# R4-viscript — báo cáo vòng 4 (chốt VS1b), dòng ViScript

Agent 2 · 2026-11-13 · `head` = `d92e947` là **head lúc thu số liệu** (QUEUE có đủ **36 hàng** 6 cột; mọi
task trước ST-31 đã `DONE`, ST-31 `READY`); commit chứa báo cáo là commit `[ST-31]` kế tiếp. Bản máy đọc:
[`R4-viscript.json`](R4-viscript.json) — **đúng 11 khoá** Khối E (`project = "viscript"`, `round = 4`).

## 1. Tóm tắt (5 dòng)

1. **VS1b chạy được đầu–cuối**: module nhiều tệp (`use ./util`, cấm vòng), `vis-std` lõi (`std.str/list/map/
   path/time/json`), **chương trình ngoài posix** (`vis-host-posix`: allowlist 8, timeout 10 s, exit≠0 ⇒
   `VS0501`), **generic** (xoá kiểu + hợp nhất call-site), **trait/`impl`/`derives`** (dispatch **tĩnh**).
2. **Hai đường trùng ngữ nghĩa**: differential **100.000 chương trình / 0 lệch** (so **đủ** `(mã, dòng, cột,
   thông điệp)`), parity **312/312** trên **338** ca sem, **0 lệch**; ca khoá
   `hai_duong_cung_so_chan_doan`.
3. **436 test workspace** (21 diag + 96 syntax + 18 fmt + **118** types + 24 interp + 50 bytecode + 9 vm +
   **100** cli); **338 ca ngữ nghĩa / 18 nhóm** (`.vis` + `.vish`, 52 `.err`).
4. **`vis test` (mới, ST-30)**: TAP `1..338`, exit 0/1/2; `vis test --vm` = **338 ok / 6 SKIP / 0 đỏ**
   (SKIP = ca cần **host chương trình ngoài** — VM không có host, ST-27). **SHA-256 thật** (VD-63) thay
   FNV-1a×4 cho hash nguồn `VBC1` **và** khoá cache; cache nạp **luôn** `load_verified`.
5. **`./vis-dev gate --fast` PASS** + **`./vis-dev nightly --fuzz-minutes 1` PASS**; khởi động ấm
   **10,3–11,1 ms** (đo trên đường `--vm`, debug build) — **đạt** §19 `< 20 ms [ƯL]`.

## 2. Bảng task vòng 4 (ST-25 → ST-31, + ST-30b)

| ID | Trạng thái | Commit | Cổng | Kết quả đo được |
|---|---|---|---|---|
| ST-25 | DONE | `4a03c6d` | fast | module `use ./util` đa tệp, cấm vòng, hash closure **gồm phụ thuộc**; sổ `07xx`; **335 test** |
| ST-26 | DONE | `3a54757` | fast | **`vis-std`** lõi: `std.str/list/map/path/time/json` (tên đầy đủ); **70** fixture std; **344 test**; sem **18 nhóm** |
| ST-27 | DONE | `a98d0e8` | fast | **`vis-host-posix`**: chương trình ngoài (allowlist 8, timeout 10 s, exit≠0 ⇒ `VS0501`), **variadic**, `--x=v`; **357 test**, **13** ca extern |
| ST-28 | DONE | `0423e67` | fast | **generic** hàm + kiểu `[T]` bằng **xoá kiểu** + hợp nhất call-site (`Hop[int] ≡ Hop[str]` — nợ §5); **392 test**, **10** ca sem `generic/*` |
| ST-29 | DONE | `7633a56` (+`520c2fe`) | fast | **trait/`impl`/`derives`**: dispatch **tĩnh** ở tầng hạ ⇒ **cùng HIR** cho interp & VM (không bảng `impls`, không bump `VBC1`); `derives Eq/Show/Json` chạy thật; **421 test** (22 + 7 ca mới) |
| ST-30 | DONE | `02b97ad` + `e11efae` | fast | **`vis test`** TAP + **SHA-256 thật** (gỡ nợ VD-54(1)); **436 test** (5 ca hash + 9 ca CLI) |
| ST-30b | DONE | `e11efae` (gộp) | fast | **đồng bộ chẩn đoán** hai đường: VM dùng `VS0503` + đúng câu interp, thêm nhánh `Bytes`, thông điệp đệ quy; differential so **đủ** `(mã, dòng, cột, thông điệp)` trên **338** ca ⇒ **312/312 khớp, 0 lệch** |
| ST-31 | READY → **báo cáo này** | — | fast | tệp này + `R4-viscript.json` (task đang viết; QUEUE@head ghi `READY`) |

**36 hàng QUEUE** khớp đúng `queue_at(d92e947)`; hàng **`ST-20b`** nay có đủ **6 cột** nên **có mặt** trong JSON
(khác R3 — hồi đó hàng này 5 cột nên `report-check` không thấy).

## 3. Số đo (mọi số tái lập bằng lệnh ghi kèm; chạy 2026-11-13, giờ UTC ghi trong ngoặc)

| Số | Giá trị | Lệnh |
|---|---|---|
| test workspace | **436** = 21 diag + 96 syntax + 18 fmt + 118 types + 24 interp + 50 bytecode + 9 vm + 100 cli | `cargo test --workspace`; theo crate `cargo test -p <crate>` (06:0xZ) |
| ca ngữ nghĩa | **338 ca / 18 nhóm** (19 thư mục, `lệnh-vish` rỗng) | `find test/sem -name '*.vis' -o -name '*.vish' \| wc -l`; `ls -d test/sem/*/` |
| ca lỗi tĩnh `.err` | **52** | `find test/sem -name '*.err' \| wc -l` |
| `vis test` (interp) | **338 ok / 0 `not ok`**, exit 0; `1..338` | `vis test` |
| `vis test --vm` | **338 ok** (**6 SKIP** = chương trình ngoài cần host) **/ 0 đỏ**, exit 0 | `vis test --vm` |
| parity chẩn đoán | **312/312 khớp, 0 lệch** (so **đủ** `(mã, dòng, cột, thông điệp)`; 26 ca bỏ qua = **lỗi biên dịch**) | `cargo test -p vis-vm --test vs_interp` |
| differential sinh tự động | `DIFF tong=100000 loi_tinh=0 chay=100000 lech=0 giay=203.5 toc_do=491/s seed=3523158054` — so **đủ** `(mã, dòng, cột, thông điệp)` (06:1xZ) | `./bin/vis-diff` (100.000; `VIS_DIFF_N` để đổi) |
| fuzz parser | **3.098.833 ca / 0 vi phạm** (60,0 s, 51.647 ca/s) — chạy **mới** trong `nightly --fuzz-minutes 1`; số **1 giờ** của ST-16 là **182.328.926 ca / 0 vi phạm** (số cũ, không chạy lại) | `./vis-dev nightly --fuzz-minutes 1`; `JOURNAL/` ST-16 |
| fuzz đột biến verifier 10 phút | **2.311.244 vòng / 0 vi phạm** (602,3 s) — **số CŨ của ST-21**, không chạy lại; cổng chạy bản ngắn (`cargo test -p vis-vm --test fuzz_mutate`, 7,08 s) | `VBC_FUZZ_PHUT=10 …` |
| VBC1 | **53 tag** (`Instr`, version 1), hash nguồn **32 byte = SHA-256** | `Instr` trong `crates/vis-bytecode/src/format.rs`; `crates/vis-bytecode/tests/hash.rs` |
| sổ mã | **7 sổ**, `all_codes()` **28** | `crates/vis-diag/tests/messages.rs` |
| SHA-256 | `sha256("abc") = ba7816bf…15ad`; `sha256("") = e3b0c442…b855`; hai nguồn khác ⇒ hash khác | `cargo test -p vis-bytecode --test hash` |
| khởi động ấm | **10,31 ms** (interp) · **10,86 ms** (`--vm` cache) · **11,08 ms** (`--vm --no-cache`) — trung vị 15 mẫu, debug build | `vis run [--vm] <tệp>` lặp 16 lần, đo bằng `perf_counter` |
| cổng | **PASS** | `./vis-dev gate --fast` (06:07Z) |
| nightly | **PASS** | `./vis-dev nightly --fuzz-minutes 1` (06:0xZ) |

**Số không tái lập được** (không ghi vào báo cáo): số token golden, số snapshot `render`, số ca tyck từng tệp —
chỉ tồn tại trong kỳ vọng của harness.

## 4. Quyết định vòng 4 (VD-57..VD-63, mỗi cái một dòng)

- **VD-57** — **module**: `use ./x` nhiều tệp, **cấm vòng**, DFS **hậu duyệt** để chốt thứ tự; chỉ `pub` mới
  xuất; sổ mã thêm **`07xx`**; hash closure **gồm mọi tệp phụ thuộc** ⇒ sửa tệp con là cache **miss**.
- **VD-58** — module **chỉ `pub`** mới nhìn thấy từ tệp khác; một đơn vị biên dịch **dùng chung** khung gốc `0`;
  `vis check` kiểm **cả** tệp con (diag gắn tệp gốc).
- **VD-59** — **`vis-std`** dùng **tên đầy đủ** (`std.str.split`) + prelude `use std.*`; `list.*`/`map.*` nhận
  `Any` (nợ §5); `json.encode` có, **`json.decode` hoãn** (VS1b).
- **VD-60** — **chương trình ngoài**: trait `Ngoai` (interp) / host (VM CLI), **allowlist 8**, timeout **10 s**
  (`VIS_CMD_TIMEOUT_MS`), exit≠0 ⇒ **`VS0501`**; **variadic** `...rest`; cờ `--x=v`; quyền `cap[Proc]` còn nợ.
- **VD-61** — **generic** = **xoá kiểu** + hợp nhất call-site: `type Hop[T]` **không** đơn hình hoá ⇒
  `Hop[int] ≡ Hop[str]`; phép toán trên `T` kiểm **động**; `vis check` = `vis run` (không còn lọc `01/02/07`
  cho tệp chính); VM dựng `entry` bằng `Call(main)`.
- **VD-62** — **trait/`derives`**: `x.m(…)` với `x` kiểu **tĩnh** `Record`/`Enum` ⇒ hạ **thẳng**
  `Call(impl_fn)` + `self` (theo giá trị) ⇒ **cùng HIR** cho interp/VM (**không** bảng `impls`, **không** bump
  `VBC1`); 3 tầng nội tại → impl tĩnh → `VS0403`; `derives Eq/Show/Json` = **kiểm + đăng ký** (chưa sinh mã).
- **VD-63** — **`vis test`** (TAP, `1..N`, exit 0/1/2) + **SHA-256 thật** cho hash nguồn/cache (gỡ nợ VD-54(1);
  không bump `VBC1`); **đồng bộ chẩn đoán** hai đường (VM `VS0503` + đúng câu interp; differential so **đủ**
  `(mã,dòng,cột,thông điệp)`); `--vm` chỉ **SKIP** ca cần host.

## 5. Lỗ hổng đã biết (đầy đủ đầu vòng 5)

1. **`Hop[int] ≡ Hop[str]`** — generic **xoá kiểu**, không đơn hình hoá (VD-61); phép toán trên `T` kiểm
   **động**; chưa có ràng buộc `T: Trait`.
2. **`&self`/`&mut self` chưa có** — `trait` phương thức dùng `self` **theo giá trị**; tham chiếu là VS1 (§3.7);
   `ref`/`ref[T]` vẫn `VS0501`.
3. **Dispatch chỉ khi kiểu TĨNH** — `x.m()` với `x` generic/`Any`/kết quả `std.list` ⇒ `VS0403` "không tĩnh";
   chưa có bảng phương thức động.
4. **Mồ côi (orphan) mới kiểm trong MỘT tệp** — `impl` chéo module chưa kiểm; **siêu trait** (`trait A: B`)
   chưa kiểm; **`derives` chưa sinh mã** (chỉ dùng ngữ nghĩa có sẵn: `Eq` cấu trúc, `Show`, `json.encode`).
5. **`| complete` chưa có** ⇒ lệnh ngoài có **exit 1** (vd `grep` không khớp) bị coi là **lỗi `VS0501`**;
   **pipe → stdin** chưa hỗ trợ; **`consent` thật/`cap[Proc]`** (Landlock) chưa có — mới allowlist + timeout.
6. **26 ca lỗi BIÊN DỊCH chưa so chẩn đoán hai đường** — differential mới so 312 ca **chạy được**; 26 ca
   `.err`-biên-dịch bỏ qua (cần harness so `vis check` ↔ đường VM).
7. **`json.decode` chưa có** (VD-59); **`list.*`/`map.*` dùng `Any`** (mất kiểu phần tử); **`std.fs` chưa có**;
   **`--desc`** (mô tả gói/cờ) chưa có; **`pm`/registry/`pkg.*`** ⇒ **VS2**.
8. **`nfree` của closure = cả khung** (thừa, ST-22 chưa tối ưu) — verifier chỉ kiểm `used ≤ nfree`.
9. **`steps` khác đơn vị** giữa interp (bước ngữ nghĩa) và VM (số lệnh) — **không** so trong differential.
10. **`match`/`if` làm đối số** chưa parse (giới hạn văn phạm §A.3); **nội suy không đệ quy** (`"{{x}}"` ⇒ `{x}`);
    `\u{{…}}` trong chuỗi chưa hỗ trợ; **`char` escape** bị từ chối `VS0105` (**theo VD-30**).
11. **`None(1)`** chưa kiểm đối số; **map literal trùng khoá** giữ cả hai; **closure snapshot** (VD-51): `var`
    cập nhật sau khi tạo closure thì closure không thấy.
12. **`vis fmt` cho `.vish`** chưa có đặc tả ⇒ `Bail out!`; **cache lợi ích ~7–8 %**, chi phối là khởi động
    tiến trình.
13. **VS2 chưa có**: `async`/`await`, kênh, net, quyền Landlock thật, LSP, gói/registry (VS2 theo §19).

## 6. Đối chiếu điều kiện chốt VS1b (§19) + đề xuất

| Điều kiện §19 (VS1) | Kết quả | Bằng chứng |
|---|---|---|
| differential interp ↔ VM trên **100.000** chương trình, 0 lệch | ✓ **100.000 chương trình / 0 lệch**, so **đủ** `(mã, dòng, cột, thông điệp)` | `./bin/vis-diff`; `JOURNAL/` (log 100k vòng 4) |
| khởi động ấm **< 20 ms [ƯL]** | ✓ **10,31–11,08 ms** trung vị (đường `--vm`, debug build) | phép đo 15 mẫu, §3 |
| bytecode từ cache **luôn** qua verifier | ✓ `load_verified` trên mọi đường nạp | `crates/vis-bytecode/src/verify.rs` |
| fuzz bytecode không panic | ✓ **2.311.244 vòng / 0 vi phạm** (ST-21; cổng chạy bản ngắn) | `JOURNAL/` ST-21 |
| generic / trait / module / `vis-std` / `vis-host-posix` **có mức chạy được** | ✓ `generic` (xoá kiểu), `trait`/`impl`/`derives` (dispatch tĩnh), module đa tệp, `std` lõi, host posix (allowlist 8, timeout) | §2; `crates/*/tests` |
| `vis run/check/fmt/test` đầy đủ | ✓ `vis test` mới (TAP, 338 ok); `vis run/check/fmt` giữ nguyên | `vis test`; `vis-dev gate --fast` |
| **Chưa đạt (VS2)**: `async`/kênh/net/quyền Landlock/LSP/gói/`json.decode`/`std.fs` | **chưa** — VS2 | §5 |

**Đề xuất cho Agent 0** (tôi **không** sửa các tệp này):
1. `AGENTS.md` §8: cập nhật **vòng 4** đã chốt (`ST-25..ST-31`, `R4-viscript`, 436 test / 338 ca / parity
   312/312) và ghi **VS1b DONE**; nêu `vis test` + SHA-256 + đồng bộ chẩn đoán (VD-63).
2. `viscript/AGENTS.md` §3/§5: dải quyết định **`VD-01..VD-63`**; thêm lệnh mới vào mục lệnh: **`vis test
   [--vm]`**, **`./bin/vis-diff`**, **`vis cache --path|--clear`**.
3. `viscript/agent/QUEUE.md`: hàng `ST-20b` **thiếu cột DoD** thứ 6 (vẫn còn) ⇒ `report-check` không thấy hàng
   này; thêm cột rồi lật `ST-31 DONE` sau khi push.
4. `docs/`: §19 nên ghi rõ "khởi động ấm" đo trên **đường `--vm`** (cache), vì `vis run` mặc định vẫn interp
   (VD-56); và ghi **`Hop[int] ≡ Hop[str]`** là giới hạn có chủ ý của VS1b (VD-61).
5. Nợ ưu tiên vòng 5: **26 ca lỗi biên dịch chưa so chẩn đoán hai đường** (lỗ hổng harness), **`&self`**,
   **`json.decode`**, **`std.fs`**, **`| complete`/pipe→stdin**, **consent/cap[Proc]**.
