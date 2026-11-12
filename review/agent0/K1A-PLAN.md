<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
> **KHÔNG PHẢI DUYỆT.** Đây là đầu vào để Agent 0 quyết. Duyệt chỉ có giá trị bằng dòng
> `APPROVED <YYYY-MM-DD> Agent 0` ghi trong `v2/agent/QUEUE.md` (AGENTS.md §8).
> Planner độc lập cho phase **K1a'**; không sửa code / QUEUE / DECISIONS / `spec/`; chỉ tạo tệp này.

# K1A-PLAN — kế hoạch phase K1a' (`vifast-objmem` + Miri)

**Lập ngày** 2026-11-13 · **HEAD khi lập** `12b8823` (dòng OS dừng ở `3da032d`) · **sổ task** `agentctl next --project v2` → *"không còn task READY"* (đã chạy, xem §8).

---

## 0. TL;DR

| Việc | Kết luận |
|---|---|
| Yêu cầu K1a' | Thay **store giả** bằng `vifast-objmem` (API closure) + **Miri từ commit đầu**; ca Untyped/CNode của KT-04c chạy trên mô hình (thiết kế `:872-873`) |
| Store giả hiện nằm ở đâu | **Hai chỗ**, không phải một: `vifast-sim::Store` (bump, ~52 dòng) và **`kobj::Machine`** giữ object trong `Vec<Object>` + watermark (1.787 dòng) — `kobj` **không** dùng `Store` |
| Blocker thật | **Miri**: có `rustup` + stable `1.99.1`, **không** có component `miri`/`rust-src`; `cargo miri` trả *"the 'miri' component … is not available for the 'stable-…' toolchain"*. Xem §3 |
| K1a' có làm được ngay? | **Không**, nếu chưa có Miri: D2-14 + QD §3.5 buộc *"Miri chạy từ commit đầu tiên của crate"* ⇒ commit tạo `vifast-objmem` phải có Miri xanh. Đây là phụ thuộc **cứng**, không phải "nice to have" |
| Đề xuất | **5 task** `KT-13..KT-17` (§4), tổng ≈ **11,5 h**, mỗi task ≤ 3 h, tuần tự `KT-13 → 14 → 15 → 16 → 17` |
| Cần Agent 0 quyết | 5 câu ở §6 — trong đó **Q1 (cài nightly+miri hay hoãn)** chặn toàn bộ vòng |

---

## 1. Yêu cầu K1a' và ràng buộc bắt buộc

**Định nghĩa xong của K1a'** (thiết kế `docs/v2/ViFastOs-v2-thiet-ke.md:872-873`):
> *thay store giả bằng objmem (API closure) + Miri* → **Miri xanh từ commit đầu**; các ca Untyped/CNode của KT-04c chạy trên mô hình.

| Ràng buộc | Nội dung | Hệ quả cho kế hoạch |
|---|---|---|
| **D2-14** | Object trong Untyped đi qua objmem, **API closure**, **Miri từ đầu** | `vifast-objmem` không được commit trước khi Miri chạy được ⇒ **KT-13 là tiền đề cứng** của KT-14 |
| **QD §3.5** (`:106-112`) | Bỏ `get2_mut`; `with/with_mut/with_many_mut` (N ≤ 8, trùng → `Alias`); con trỏ thô + `UnsafeCell`, **không bao giờ tạo tham chiếu tới cả vùng**; sim cấp vùng bằng **bộ cấp phát thô có căn lề, không `Vec<u8>`**; bỏ `refs` khỏi header; **Miri từ commit đầu tiên** | Chữ ký 5 method ở thiết kế `:204-210` là hợp đồng; test căn lề + alias từng cặp là DoD, không phải test phụ |
| **D2-17** | `unsafe` chỉ trong danh sách crate đóng | Luật **R5** đã allowlist `*vifast-objmem*` (`v2/bin/v2:161`) ⇒ **không phải sửa luật** khi thêm crate. Nhưng `v2/sim/**` **không** nằm trong allowlist ⇒ phần tích hợp trong sim phải **safe 100%**, chỉ gọi API objmem |
| **D2-37** | Agent 2 **không** ghi QUEUE; `agentctl done` chỉ là đề xuất | Kế hoạch này không đề xuất ai ghi QUEUE; việc lật sổ do Agent 0 |
| **D2-07** | Không retry để qua cổng; flake = `T-INFRA` | DoD ghi "PASS không retry"; Miri timeout là `T-INFRA`, không phải lý do bỏ Miri |
| **D2-05** | Commit chạm `kernel/`, `lib/` phải xanh `--fast`; lỗi logic phải có test đỏ trước | KT-14..16 đều chạm `lib/` ⇒ `gate --fast` bắt buộc cuối mỗi task |
| **Thiết kế `:743`, `:1103`** | objmem: **Miri toàn bộ test**; **fuzz chuỗi thao tác**; test `with_many_mut` kiểm **alias từng cặp**; cấm tạo tham chiếu tới cả vùng | Tách fuzz + alias thành task riêng (KT-15) để KT-14 vừa 3 h |
| **Thiết kế `:793`, `:714`, `:684`** | `vifast-model` là *"kernel ngây thơ bằng std …, **không objmem**"*; `objmem` chỉ dùng cho **kernel + sim**; crate `lib/` là `no_std` | **K1a' KHÔNG đụng `vifast-model`** (crate đó còn `#![forbid(unsafe_code)]`, `src/lib.rs:25`). Mô hình giữ vai **oracle**; "chạy trên mô hình" của K1a' = parity vẫn xanh sau khi sim đổi ruột |
| **R1** (`v2/bin/v2:101`) | Tệp trong `lib/` ≤ **1.500 dòng** | Mục tiêu objmem **< 800 dòng** [ƯL] (thiết kế `:198`) ⇒ an toàn, nhưng không nhồi fuzz vào `src/` |

---

## 2. Hiện trạng code (đo được, không suy đoán)

| Thành phần | Số dòng | Vai trò với K1a' |
|---|---|---|
| `v2/sim/vifast-sim/src/lib.rs::Store` (`:45-96`) | ~52 | Bump allocator "store giả" của KT-08; **chỉ** dùng cho `Prog::Alloc` (harness), trả địa chỉ giả `u64` |
| `v2/sim/vifast-sim/src/kobj.rs` | **1.787** | **Đây mới là store giả của object**: `objects: Vec<Object>` (`:396-399`), watermark từng Untyped (`:832-935`), `Kind` chứa `Vec`/`VecDeque` (`:347-372`) |
| Điểm truy cập trực tiếp | **15×** `self.objects[…]`, **17×** `self.caps[…]`, **11×** `&(mut) self.objects` | Bề mặt refactor; đã có helper `object()/object_mut()/cap()/push_object()` (`:875-899`, `:1300`) để gom |
| Hợp đồng công khai | 7 tệp test (`kt09_*`, `kt10_*`, `kt11_*`) + `scenario.rs:20` | **Không được đổi chữ ký** `Machine` ⇒ test cũ chạy nguyên si là bằng chứng hồi quy |
| `v2/lib/vifast-model` | 1.105 + 145 + 111 + 39 | Oracle; `kt11_diff_sim.rs` parity 10.000 seed; dev-dep **ngược** sang `vifast-sim` ⇒ đổi sim phải giữ parity |
| `.scn` | 21 tệp; 9/9 ca Untyped/CNode chạy qua `kt11_scn.rs` | Phải vẫn 9/9 sau K1a' (D2-46) |

**Kết luận phạm vi**: "thay store giả" = thay **cả hai** (`Store` + ruột `kobj::Machine`). Nếu chỉ thay `Store` thì D2-14 **không** được đáp ứng — `Store` không chứa object nào.

---

## 3. Blocker toolchain (kiểm lại read-only, có log)

### 3.1 Đo được

| Lệnh | Kết quả |
|---|---|
| `which rustc cargo rustup` (PATH mặc định của shell) | **rỗng** — `~/.cargo/bin` không có trên PATH non-login |
| `~/.cargo/bin/rustup --version` | `rustup 1.29.3 (edbc831ce 2026-11-02)` |
| `rustup toolchain list` (trước khi kiểm) | `stable-aarch64-apple-darwin` (active), `1.99.1-aarch64-apple-darwin` |
| `rustc -V` | `rustc 1.99.1 (58cf8f9c3 2026-10-13)` |
| `cargo miri --version` | **`error: the 'miri' component which provides the command 'cargo-miri' is not available for the 'stable-aarch64-apple-darwin' toolchain`** |
| `rust-toolchain.toml` | `channel = "stable"`, components `rustfmt`,`clippy`, targets `x86_64/aarch64-unknown-none`, `riscv64gc-unknown-none-elf` |
| `miri-preview` cho `aarch64-apple-darwin` | **có** trong manifest nightly (`static.rust-lang.org/dist/channel-rust-nightly.toml:1404`, `available = true`) |
| Mạng tới `static.rust-lang.org` | HTTP **200** (0,16 s) ⇒ cài được về mặt kỹ thuật |

### 3.2 Sự cố đã xảy ra trong lúc kiểm tra (khai báo, không giấu)

Lệnh **kiểm tra** `cargo +nightly …` đã khiến **rustup tự cài** toolchain `nightly-aarch64-apple-darwin`
(`profile = "minimal"` trong `~/.rustup/settings.toml` ⇒ chỉ có `rustc`, `cargo`, `rust-std`, `rustdoc`;
`birth=2026-11-13T03:15:12Z`). Đây là **tác dụng phụ của rustup auto-install**, không phải cài đặt có chủ ý;
trước đó máy chỉ có stable. Hiện trạng **bây giờ**: `rustc 1.102.0-nightly (3ef0c22cd 2026-11-11)` **có**,
`miri` + `rust-src` **vẫn thiếu** ⇒ KT-13 rút ngắn còn ~30–45 phút (chỉ `rustup component add`).
Không có agent nào khác đang chạy khi sự việc xảy ra (đã kiểm `list_agents`).

### 3.3 Hai lựa chọn — Agent 0 phải chọn, không được lặng lẽ bỏ D2-14

| | **A. Cài `miri` + `rust-src` cho nightly** *(khuyến nghị)* | **B. Hoãn Miri, ghi YC** |
|---|---|---|
| Việc làm | `rustup component add miri rust-src --toolchain nightly`; `cargo +nightly miri setup` | Không cài; chỉ làm phần không tạo commit objmem |
| Chi phí | ~2–4 phút, ~60–100 MB, cần **mạng + quyền ghi `~/.rustup`** | 0 |
| Hệ quả | K1a' chạy đủ 5 task; DoD K1a' đóng được | **KT-14..KT-17 BLOCKED**; K1a' **không thể DONE**. Chỉ còn KT-13 phần "ghi nhận thiếu" |
| Rủi ro | Miri chậm (test objmem nhỏ nên OK); nightly đổi hành vi lint | Phải có **D2 mới** sửa thứ tự D2-14 ("Miri sau") — chỉ Agent 0 được quyết |
| Không được | — | **Không** đánh dấu K1a' DONE, **không** ghi "Miri N/A" |

> Ghi chú kỹ thuật đã kiểm: `+nightly` **thắng** `rust-toolchain.toml` (rustup đọc file chỉ ở nhánh không có `+`);
> nên `cargo +nightly miri test` **không** kéo theo 3 target bare-metal của file pin, và **không** cần sửa
> `rust-toolchain.toml` (giữ stable cho `fmt`/`clippy`/`gate`).

---

## 4. Bảng task đề xuất (K1a' — không vượt phase)

| ID | Mục tiêu | Phụ thuộc | Ngân sách | Trạng thái đề xuất |
|---|---|---|---|---|
| **KT-13** | Bootstrap Miri + lệnh `v2 miri` (cổng chạy được cho mọi commit objmem) | — | **1,5 h** | READY (sau Q1=A) |
| **KT-14** | `vifast-objmem` core: `ObjHeader`/`ObjRef`/`ObjStore` + arena thô căn lề + `place/with/with_mut/destroy` | KT-13 | **3 h** | chờ KT-13 |
| **KT-15** | `with_many_mut` N≤8 + kiểm alias từng cặp + fuzz chuỗi thao tác | KT-14 | **2 h** | chờ KT-14 |
| **KT-16** | Tích hợp objmem vào sim: thay `Store` **và** ruột `kobj::Machine`; giữ nguyên API | KT-14, KT-15 | **3 h** | chờ KT-15 |
| **KT-17** | Cổng Miri tổng + KT-04c trên objmem + chốt DoD K1a' | KT-13..16 | **2 h** | chờ KT-16 |

Tổng **11,5 h** (≈ 4–5 phiên). Đường găng tuyến tính; không có task nào song song được (cùng crate `vifast-objmem` / cùng tệp `v2/bin/v2` → tránh trùng write-scope).

### KT-13 — Bootstrap Miri + `v2 miri`

- **Test đỏ trước**: `./v2/bin/v2 miri` → `v2: lệnh không biết: miri`, **exit 2** (đã chạy, §8) và
  `cargo miri --version` → *"component … is not available"* (đã chạy). Dán cả hai log vào `JOURNAL`.
- **DoD đo được**:
  1. `rustup component list --installed --toolchain nightly` có **`miri`** và **`rust-src`**; `cargo +nightly miri --version` in phiên bản.
  2. `./v2/bin/v2 miri -p vifast-sim --lib` **PASS** (PoC thật: 7 unit test thuần tính toán, không I/O, không `Instant` — đã kiểm `v2/sim/**` **không** dùng `std::time`/`thread`/`fs`).
  3. Khi thiếu toolchain, `v2 miri` **fail rõ ràng** (exit ≠ 0 + in đúng câu lệnh bootstrap) — **không** im lặng bỏ qua.
  4. `./v2/bin/v2 check` + `gate --fast` vẫn PASS (không hồi quy wrapper).
- **Tệp**: `v2/bin/v2` (thêm 1 subcommand + usage; không đụng 5 luật R1..R5), `v2/agent/STATE.md`, `v2/agent/JOURNAL/`.
- **Rủi ro**: (a) Miri cần `rust-src` — thiếu là `miri setup` fail; (b) chạy nhầm test có I/O dưới Miri cần `-Zmiri-disable-isolation` ⇒ PoC chọn `--lib` của sim; (c) wrapper đang là "cổng cứng" — sửa phải giữ nguyên exit code các lệnh cũ.
- **BLOCKED khi**: `rustup component add` lỗi mạng/quyền → `BLOCKED` + YC, **không** chuyển sang "Miri N/A".

### KT-14 — `vifast-objmem` core

- **Test đỏ trước**: `cargo test -p vifast-objmem` → `error: package ID specification 'vifast-objmem' did not match any packages`, **exit 101** (đã chạy, §8). Sau khi thêm member: test căn lề/`Dead`/`destroy` đỏ vì chưa có code.
- **DoD đo được** (mỗi mục là 1 test hoặc 1 lệnh):
  1. `ObjHeader` `#[repr(C)]`, **16 B** (`kind: u8`/`state: u8`/`_pad: u16`/`id: u64`), **không** có trường `refs` (QD §3.5).
  2. Chữ ký **đúng** thiết kế `:204-210`: `with`, `with_mut`, `with_many_mut<const N>`, `unsafe place`, `unsafe destroy`; lỗi `Dead`, `Alias`.
  3. `ObjRef<T: ?Sized> { pa, id, PhantomData }`; `id` **không tái dùng** (counter đơn điệu; test 3 vòng place/destroy → 3 id khác nhau).
  4. Arena **thô + căn lề**, **không** `Vec<u8>`: cấp `Layout` align 8/16/64 → địa chỉ trả về chia hết; cấp quá vùng → `None`/`Err`, không panic.
  5. `with`/`with_mut` chạy closure, trả `Ok`; ref có `id` lệch header → **`Dead`**; ref sau `destroy` → **`Dead`**; `destroy` gọi `drop_in_place` (test đếm `Drop` của một kiểu đếm được) và **hai lần destroy** không UB.
  6. `#![no_std]` + **build bare-metal xanh**: `cargo build -p vifast-objmem --target aarch64-unknown-none` (target đã cài sẵn cho stable).
  7. **Miri xanh ngay commit đầu**: `./v2/bin/v2 miri -p vifast-objmem` PASS (D2-14). Log dán `JOURNAL`.
  8. `gate --fast` PASS, không retry (D2-07).
- **Tệp**: `v2/lib/vifast-objmem/{Cargo.toml, src/lib.rs, src/obj.rs, src/arena.rs, src/store.rs}`, `v2/Cargo.toml` (+1 member), `v2/agent/{STATE.md,JOURNAL/}`. **Không** chạm `src/` khác.
- **Rủi ro**: (a) Miri bắt Stacked-Borrows ở ranh giới `place`/`destroy` (đúng mục đích — sửa, không `#[allow]`); (b) `no_std` + test cần `std` ⇒ `#![cfg_attr(not(test), no_std)]`; (c) SPDX R3 trong 5 dòng đầu mọi tệp `.rs`/`.toml`; (d) `cfg(target_arch)` bị R2 cấm trong `lib/`.
- **BLOCKED khi**: Miri đỏ mà nguyên nhân là **thiết kế API** (không phải bug) → dừng, 2 lựa chọn + `file:line` cho Agent 0; hoặc vượt 3 h.

### KT-15 — `with_many_mut` + alias từng cặp + fuzz

- **Test đỏ trước**: 3 test alias (cùng `pa`, cùng `pa` khác `id`, chồng một phần) — biên dịch đỏ vì chưa có method.
- **DoD đo được**:
  1. `[ObjRef<dyn KObj>; N]` với `N ≤ 8` cưỡng chế **lúc biên dịch** (`const { assert!(N <= 8) }`), `N=9` **không** build (ghi log đỏ).
  2. Ma trận `N = 1..8` × 3 kiểu trùng ⇒ `Alias`; `N` ref rời nhau ⇒ `Ok`, closure ghi được vào **từng** object.
  3. Miri xanh cho **toàn bộ** test alias (đây là ca Miri quan trọng nhất của phase).
  4. Fuzz **tất định** từ seed: **10.000** chuỗi × ~32 thao tác (`place/destroy/with/with_mut/with_many_mut`) trên stable; bất biến sau **mỗi** bước: *không object sống nào chồng lấn*, `arena.used()` khớp tổng block đã cấp, mọi `ObjRef` sống đều `Ok`, mọi ref đã destroy đều `Dead`.
  5. Bản rút gọn **200 chuỗi** chạy **dưới Miri** (không nới ngưỡng: 2 con số 10.000/200 ghi rõ trong test, `10.000` là DoD của stable).
- **Tệp**: `v2/lib/vifast-objmem/src/many.rs`, `v2/lib/vifast-objmem/tests/{alias.rs, fuzz.rs}`, `v2/agent/{STATE.md,JOURNAL/}`.
- **Rủi ro**: (a) Miri 200 chuỗi có thể > 20 phút ⇒ nếu vượt, **giảm số chuỗi Miri** (ghi lý do), **không** giảm 10.000 của stable; (b) fuzz dùng RNG tự viết (không thêm dependency — D2-21/allowlist).
- **BLOCKED khi**: cần dependency RNG ngoài allowlist → dừng, hỏi Agent 0 (không tự thêm `rand`).

### KT-16 — Tích hợp objmem vào sim (thay store giả)

- **Test đỏ trước**: `v2/sim/vifast-sim/tests/kt16_objmem.rs` (mới): sau `retype`, object phải có địa chỉ **trong** `[arena.base, arena.base+cap)`; `arena.used()` khớp tổng byte object; cap trỏ object đã `Revoke` → truy cập trả `Dead`; `Machine::new()` cấp arena (mặc định **1 MiB** [ƯL], có `with_arena_bytes`). Trước tích hợp: đỏ (không có API/không nằm trong arena).
- **DoD đo được**:
  1. **Không sửa một dòng** test cũ: `git diff --stat v2/sim/vifast-sim/tests/kt09* kt10* kt11*` **rỗng**; 7 tệp đó vẫn xanh.
  2. `kobj::Machine` giữ **nguyên chữ ký công khai**; object body nằm trong objmem; **không** còn `Vec<Object>` làm nơi chứa object.
  3. `v2/sim/**` **không** có `unsafe` (R5 vẫn xanh — sim không thuộc allowlist D2-17).
  4. L1 **1.000 seed** xanh, **≤ 60 s** (AGENTS §6); `gate --fast` PASS không retry.
  5. `kt11_diff_sim` parity **10.000 seed / 0 lệch** (oracle `vifast-model` **không** bị sửa).
  6. `.scn` Untyped/CNode **9/9** xanh (D2-46).
  7. Miri xanh cho `vifast-objmem` + `kt16_objmem`; `kt10_cnode` chạy Miri được (ghi nhận nếu phải loại trừ vì thời gian).
- **Tệp**: `v2/sim/vifast-sim/src/kobj.rs`, `src/lib.rs` (`Store` → arena), `v2/sim/vifast-sim/Cargo.toml` (+path dep), `tests/kt16_objmem.rs`, `v2/agent/{STATE.md,JOURNAL/}`.
- **Bước 0 bắt buộc (≤ 45′, trong ngân sách)**: gom **mọi** truy cập `self.objects[…]`/`self.caps[…]` qua helper `object()/object_mut()/cap()/cap_mut()` (refactor **no-op**, test cũ phải xanh trước khi đổi ruột). Nếu bước 0 chưa xong trong 45′ ⇒ dừng, tách như dưới.
- **Rủi ro**: (a) `Kind` chứa `Vec`/`VecDeque` ⇒ `place` phải `ptr::write`, `destroy` phải `drop_in_place`, không được move qua `&`; (b) closure API + `&mut self` dễ vướng borrow ⇒ mọi thao tác nhiều object phải qua `with_many_mut`; (c) ~43 điểm truy cập trực tiếp là nơi dễ sinh hồi quy ⇒ revert trước nếu đỏ (D2-03); (d) arena 1 MiB × 10.000 seed (parity) có thể chậm ⇒ đo, dùng lại arena nếu cần.
- **BLOCKED khi**: vượt 3 h ⇒ **tách đã định trước**: `KT-16a` = refactor helper + `Store` → `objmem::Arena` (2 h), `KT-16b` = thay `Vec<Object>` bằng placement (3 h). Cần Agent 0 đồng ý tách **trước** khi làm (Q4).

### KT-17 — Cổng Miri tổng + KT-04c trên objmem + chốt DoD K1a'

- **Test đỏ trước**: `./v2/bin/v2 nightly` hiện là **stub**: in *"nightly CHƯA CÓ (cần mutation/Miri/soak…)"*, **exit 2** (`v2/bin/v2:316-319`) ⇒ chưa có cổng Miri tổng.
- **DoD đo được**:
  1. `./v2/bin/v2 nightly --only-miri` (mới) chạy Miri cho `vifast-objmem` **+** tập test sim đã chọn (kt09_untyped, kt09_revoke, kt10_cnode, kt16_objmem) → **PASS**, log lưu `v2/agent/JOURNAL/`.
  2. `nightly` **không** bị tuyên bố "đã có" cho mutation/soak/L1-100k (vẫn ngoài K1a'); usage ghi rõ phần nào có.
  3. Chạy lại **9/9** `.scn` Untyped/CNode trên sim objmem + parity 10.000 seed, dán log.
  4. `gate --fast` PASS **không retry**; `reports/` (nếu Agent 0 yêu cầu) cập nhật theo Khối E.
  5. Bảng "K1a' DoD" trong `STATE.md`: 2 dòng (Miri từ commit đầu; ca KT-04c trên mô hình) đều **có bằng chứng**.
- **Tệp**: `v2/bin/v2`, `v2/agent/{STATE.md,JOURNAL/,reports/}`.
- **Rủi ro**: (a) Miri toàn bộ `kt10_cnode` (13 test, property 6.144 thao tác) rất chậm ⇒ chọn tập con có lý do ghi rõ, **không** giảm phạm vi objmem; (b) AGENTS §6: `nightly`/soak **không** chạy song song với build/cổng khác.
- **BLOCKED khi**: Miri phát hiện UB thật trong code đã commit ⇒ **revert trước** (D2-03), mở `JOURNAL` + `.scn`/test tái hiện, rồi mới điều tra.

---

## 5. Thứ tự, cổng, và bằng chứng mỗi task

```
KT-13 (Miri)  →  KT-14 (objmem core)  →  KT-15 (alias+fuzz)  →  KT-16 (tích hợp sim)  →  KT-17 (cổng Miri + DoD)
   1,5 h              3 h                     2 h                    3 h                     2 h
```

- Mỗi task kết thúc bằng: `./v2/bin/v2 check` → `gate --fast` → (KT-14 trở đi) `./v2/bin/v2 miri` → commit `[KT-##]` → `STATE`/`JOURNAL`.
- Mỗi task có **test đỏ trước** dán log (§4) — Khối C mục 2 của QUEUE.
- Không task nào chạm `spec/`, `QUEUE.md`, `DECISIONS.md`, `AGENTS*`, `viscript/`, `docs/`.

---

## 6. Câu hỏi cần Agent 0 quyết

| # | Câu hỏi | Đề xuất mặc định | Chặn gì |
|---|---|---|---|
| **Q1** | Cài `miri` + `rust-src` cho nightly (**A**) hay hoãn Miri + ra YC sửa thứ tự D2-14 (**B**)? | **A** — đã có nightly, còn ~2–4 phút; B làm K1a' không thể DONE | **Toàn bộ KT-14..17** |
| **Q2** | Miri đặt ở đâu: lệnh rời `v2 miri` (mỗi commit) + `nightly --only-miri`, **không** vào `gate --fast`? | Đúng như vậy — Miri chậm, `--fast` phải giữ ≤ 60 s (AGENTS §6) | KT-13, KT-17 |
| **Q3** | D2-37(c) nói *"cargo-deny đầy đủ từ mốc đó/K1a"*: K1a' có **bắt buộc** thêm `deny.toml` + bước kiểm "không registry dependency" không? Hiện **chưa có** `deny.toml`; `vifast-objmem` **không** có dependency ngoài. | **Chưa bắt buộc** ở K1a' (không thêm dep ngoài); thêm `deny.toml` thành task riêng khi có dep đầu tiên | KT-14 (nếu bắt buộc ⇒ +1 task 1 h) |
| **Q4** | Đồng ý **tách trước** `KT-16a`/`KT-16b` nếu tích hợp vượt 3 h? | Có — tách theo ranh giới đã định ở §4, không gộp | KT-16 |
| **Q5** | Phạm vi K1a': chỉ **object body** vào objmem; bảng `Cap` của sim vẫn là `Vec<Cap>` (tiện ích mô hình, không phải object kernel) — đúng ý D2-14? | Đúng — CSpace 2 cấp (đưa cap vào CNode slot) là K1b/K2, không phải K1a' | KT-16 (nếu sai ⇒ KT-16 phình, phải tách thêm) |

---

## 7. Ngoài phạm vi K1a' (không đề xuất, không làm)

- K1b (TCB/scheduler/MCS), K1c (IPC/Reply/Notification/Exit), K2 (VSpace/Frame) — **không** mở.
- **Không** sửa `vifast-model`: `#![forbid(unsafe_code)]` + thiết kế `:793` *"không objmem"*. Model là **oracle**; nếu KT-16 làm parity lệch thì **sửa sim**, không sửa model để khớp.
- **Không** mutation testing / soak / L1 100.000 seed (thuộc `nightly` đầy đủ, phase sau).
- **Không** đổi `spec/abi.toml`, không đổi ABI, không đổi `.scn`.
- **Không** nới bất kỳ ngưỡng nào (seed, thời gian, `MAX_*`, số ca đỏ).

---

## 8. Bằng chứng lệnh đã chạy (read-only, trừ sự cố §3.2)

```text
$ git rev-parse --short HEAD                     → 12b8823   (3da032d là tổ tiên; 9 commit dòng ViScript sau đó)
$ cargo run -p vifast-agentctl -- next --project v2
  agentctl: không còn task READY có phụ thuộc DONE
$ rustc -V                                       → rustc 1.99.1 (58cf8f9c3 2026-10-13)
$ rustup --version                               → rustup 1.29.3 (edbc831ce 2026-11-02)
$ cargo miri --version                           → error: the 'miri' component which provides the
                                                   command 'cargo-miri' is not available for the
                                                   'stable-aarch64-apple-darwin' toolchain
$ ./v2/bin/v2 miri                               → v2: lệnh không biết: miri          [exit 2]
$ cargo test -p vifast-objmem                    → error: package ID specification `vifast-objmem`
                                                   did not match any packages              [exit 101]
$ cargo test -p vifast-sim --no-run              → OK (baseline stable biên dịch xanh)
$ curl -s -o /dev/null -w '%{http_code}' https://static.rust-lang.org/dist/channel-rust-nightly.toml → 200
```

Kiểm tra thêm: `v2/sim/**` **không** dùng `std::time`/`thread`/`fs` ⇒ Miri chạy được không cần
`-Zmiri-disable-isolation`; `kt11_scn.rs:135` **có** đọc tệp ⇒ **không** đưa test đó vào Miri.
