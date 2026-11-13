<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
> **Đây không phải nguồn sự thật và không phải duyệt.** Đây là gói đề xuất để Agent 0 quyết
> trong một lượt. Duyệt chỉ có giá trị bằng dòng `APPROVED <YYYY-MM-DD> Agent 0` trong
> `viscript/agent/QUEUE.md` (AGENTS.md §8). Soạn 2026-11-13 16:2x JST, HEAD `0e65446` (**482 test**; ST-33/ST-34 vừa xong).

# Issue #21 → **VSH1a**: bộ lệnh shell có cấu trúc (bản chờ ký)

Nguồn: <https://github.com/vifast-oss/ViFastOs/issues/21> (`khoa-pham-aepsy`, 2026-11-13).
Đối chiếu: `docs/viscript/ViScript-thiet-ke.md` §9/§12/§13/§19, `viscript/agent/{QUEUE,DECISIONS,STATE}.md`,
code `viscript/crates/**` ở HEAD `0e65446`, binary `viscript/target/debug/vis`.

**Ràng buộc mới do người điều phối nêu (2026-11-13):** ViScript đang được làm **lặp để ra hình thái
cơ bản trước**, và **tương lai sẽ tách thành repository riêng**. Mọi quyết định dưới đây ưu tiên:
(i) mỗi vòng ra **một hình thái chạy được**, (ii) không tạo ràng buộc chéo `viscript/` ↔ phần còn lại
của ViFastOs, (iii) mọi thứ thuộc ngôn ngữ phải **nằm trong `viscript/`** để `git filter-repo` là tách được.

---

## 0. Kết luận 6 dòng

1. Nhận hướng của issue, nhưng **không** nhận cách chia P1..P4; thay bằng **VSH1a** — vòng 6, 8 hàng (7 mã task; ST-40 tách a/b).
2. VSH1a = **nửa đầu của mốc VSH1** trong lộ trình §19 (VSH1 và VS2 là anh em, cùng phụ thuộc VS1) ⇒
   không phải "việc chen ngang", cũng không phải chờ VS2. Làm **sau ST-36** (R5).
3. ST-34 (`| complete` + pipe→stdin) đã xong `4ababc6`; ST-35 (generic thật) là task kế. P1 phải làm trên
   **chữ ký lệnh generic** (khe ống hiện khớp `==`), không thì `take`/`where` không nối được. Đây là task
   đầu tiên, không phải "động từ thuần".
4. Semantics của `where/each/first/count` **đã có** trong bảng phương thức (`filter/each/first/len`) và
   đã kiểu hoá đúng; verb chỉ là **alias có chữ ký**, một ngữ nghĩa hai tên (VD-70).
5. `get`/`select` (chiếu theo trường) và `it` + viết tắt trường **phải đi cùng bản ghi**, tức cùng
   `FileEntry`/`ProcEntry` prelude — gộp thành một nhóm task, không tách như issue.
6. `ps`/`kill` **không thuộc ViScript**: interface chốt trong ViScript (VD-73), cài đặt ở
   `vis-host-vifast` (ngoài `viscript/`, dòng OS) khi có `procsrv`. Không làm ở vòng 6.

---

## 1. Quyết định 1 — Vị trí lộ trình: **VSH1a, vòng 6, sau ST-36**

§19 xếp `VSH1 ← VS1`, `VS2 ← VS1`: VSH1 **không** phải sau VS2. VSH1 gồm "chế độ lệnh đầy đủ,
stream, chương trình ngoài, gợi ý theo kiểu, lịch sử, job". Issue #21 xin đúng **nửa đầu** của VSH1
(bộ lệnh có cấu trúc + hiển thị), nên đặt vào VSH1 là đúng chỗ, không phải "đề xuất ngoài lộ trình".

| Mốc | Nội dung vòng 6 (VSH1a) | Không làm ở vòng 6 (VSH1b/VS2) |
|---|---|---|
| Bộ lệnh | `where/each/first/count/sort_by` (typed), `ls`, `open` | `walk`, `save`, `to json`, `from json[T]`, `http` |
| Hiển thị | `Render`: bảng `list[bản ghi]`, khối khoá–giá trị | ViTerm có cấu trúc, màu, cắt theo bề rộng terminal |
| Bản ghi | prelude `FileEntry`/`ProcEntry` + `it`/viết tắt trường | `datetime`/`instant` thật, `cap[*]` cưỡng chế |
| Shell | — | REPL/ViShell, job control, `ps`/`kill` chạy thật, stream lười |
| Nguồn | verb là builtin có chữ ký + seam `SigSource` | `std.shell` **viết bằng ViScript** (module nhúng) |

**Hệ quả cho issue:** "chạy được ngay trên Mac/Linux từ bây giờ" = đúng ở mức `vis run tệp.vish`
(có pipeline typed, có bảng). "Dùng hằng ngày trong ViShell" vẫn chờ VSH1b (REPL) — nói rõ trong trả lời,
đừng để người đề xuất hiểu là shell dùng được ngay sau vòng 6.

---

## 2. Quyết định 2 — Ranh giới ViScript ↔ ViFastOs (vì sẽ tách repo)

Nguyên tắc: **viết trong `viscript/` = thuộc ngôn ngữ; chạm hệ điều hành = thuộc backend**. ViScript
định nghĩa *hợp đồng*, ViFastOs *cài đặt hợp đồng*. Không có chiều ngược lại.

| Hạng mục | Chủ | Nơi ở | Ghi chú cho việc tách repo |
|---|---|---|---|
| `Type`, prelude `FileEntry`/`ProcEntry`, `Render`, `CmdSig` generic | **ViScript** | `viscript/crates/vis-types` | Ổn định hoá thành "stdlib ABI", ghi trong `viscript/agent/DECISIONS.md` (đi theo repo) |
| `trait Host` + `FsHost` + `ProcsHost` (mới) | **ViScript** | `viscript/crates/vis-interp/src/builtin.rs` | Thêm `HOST_API: u32` (tăng khi đổi trait) để backend ngoài biết mình lệch |
| `vis-host-posix` | **ViScript** | `viscript/crates/vis-host-posix` | Backend mẫu, hermetic test |
| `vis-host-vifast` (RPC `procsrv`, quyền thật) | **ViFastOs** | workspace **ngoài** `viscript/` (thiết kế §12; QUEUE Khối A) | Không được xuất hiện đường dẫn kernel trong `viscript/` |
| `ps`/`kill`, `cap[Proc]`, Landlock/procsrv | **ViFastOs** (VS2) | `vis-host-vifast` + dòng OS | Interface chốt ở VD-73; cài đặt là task `KT-*` sau |
| `docs/viscript/*` (thiết kế, SPEC-lex) | **ViScript** | hiện ở `docs/viscript/` (**ngoài** `viscript/`) | **Nợ tách repo**: 11 tệp test, 18 chỗ đọc `../../../docs/viscript/…`; khi tách phải chuyển `docs/viscript/` vào `viscript/docs/` hoặc bật biến môi trường — ghi thành task riêng, không sửa bây giờ |

Luật bổ sung cho vòng 6 (không cần sửa AGENTS.md):

- Mọi tệp mới của vòng 6 **nằm dưới `viscript/`**; không sửa `crates/` ở gốc repo.
- `ProcEntry` chỉ chứa trường **portable** (`pid, ppid, name, cpu: percent, mem: size, state`), **không**
  có khái niệm kernel (`tcb`, `service`, `cap`). ViFastOs tự ánh xạ.
- Không backend nào được gọi trực tiếp từ `vis-interp`/`vis-vm` (đã là luật; vòng 6 thêm ca âm khoá lại).
- Sổ `viscript/agent/DECISIONS.md` là nơi ghi **hợp đồng** (đi theo repo khi tách); `docs/` đóng băng vẫn
  là nguồn thiết kế, **không** sửa.

---

## 3. Quyết định 3 — Cách cài: chữ ký **có kiểu** + builtin alias + seam `SigSource`

Ba đường, chọn một:

| Đường | Ưu | Nhược | Kết luận |
|---|---|---|---|
| **(A)** `CmdSig` generic + verb là builtin gọi lại `builtin::method` | Kiểu thật (`5mb` định kiểu theo trường); một ngữ nghĩa; interp/VM dùng chung ⇒ differential 0 lệch; không cần hạ tầng module | Phải mở generic cho `CmdSig` | **CHỌN** |
| (B) Verb = hàm `std.list.*` kiểu `Any` (như `list.sort_by` hiện tại) | Nhanh nhất | Mất kiểm kiểu ⇒ tiêu chí "sai kiểu ⇒ VSxxxx" của issue không đạt; đóng băng nợ `Any` ngay trên mặt shell | Không |
| (C) `std.shell` viết bằng ViScript (module nhúng) | Đúng §13 "viết bằng ViScript" | Cần hạ tầng module nhúng + `cmd` trong module + merge chữ ký; chậm ra hình thái | Hoãn — nhưng **chừa seam** để đổi sau mà không đổi cú pháp |

**Seam `SigSource`**: `CmdSig` thêm `nguon: SigSource { Builtin, FileCmd, ModuleCmd }`; việc tra chữ ký
không được hard-code "builtin ⇒ Rust". Vòng 6 chỉ dùng `Builtin`/`FileCmd`; khi tách repo, `ModuleCmd`
cho phép chuyển verb sang ViScript mà `.vish` của người dùng **không đổi một byte**.

Chi tiết kỹ thuật đã kiểm chứng ở HEAD:

- Khe ống khớp **bằng đúng** (`p.ty == *up_ty`) — `viscript/crates/vis-types/src/lower/cmd.rs:376`;
  `take` khai `list[str]` ⇒ ví dụ §9.7 chết ở `take` (`VS0603 … không có khe nhận error`).
- `closure_arg` parse được (`CmdClosure`) nhưng **không có nhánh nào** trong `cmd_lower`, và 0 fixture
  `.vish` dùng `=>`; parser **không** giữ cây con đã parse (chỉ giữ token) ⇒ hạ phải re-parse span
  (đúng §A.7 "phân giải ở `cmd_lower`") và **cộng offset span** để báo lỗi đúng `dòng:cột`.
- Semantics đã có + đã kiểm kiểu: `filter/each` đòi hàm 1 tham số, `filter` đòi trả `bool`
  (`tyck/expr.rs:767`), `first`/`get` trả `T?`; `list.sort_by` khoá **`int`**, sắp xếp **ổn định**
  (`vis-interp/src/stdlib.rs:228`), kiểu khoá không kiểm tĩnh (`Ty::FnIntInt → None`).
- Đã có tiền lệ **stage giả** (`| complete`, `lower/cmd.rs:341`): nếu `get <tên trường>` (chọn trường
  trần, không phải biểu thức) cần dạng riêng thì có mẫu để cài **không mở HIR**; nhưng vẫn ưu tiên hạ
  trọn trong `cmd_lower`.
- `sig_builtin()` **không** biểu diễn được `Type::Record(RecId)` (id do `TypeTable` của tệp cấp) ⇒
  prelude bản ghi là hạ tầng bắt buộc, không phải "nice to have".
- `std.list/map/…` khai `Ty::Any` có chủ ý (VD-59 (3)) ⇒ **không** lấy làm mặt shell.

Ánh xạ verb → ngữ nghĩa (một ngữ nghĩa, hai tên):

| Verb | Tương đương | Kiểu (sau vòng 6) |
|---|---|---|
| `where` | `.filter` | `where[T](xs: list[T], f: fn(T) -> bool) -> list[T]` |
| `each` | `.each` | `each[T](xs: list[T], f: fn(T) -> ()) -> ()` |
| `first` | `.first` | `first[T](xs: list[T]) -> T?` |
| `count` | `.len` | `count[T](xs: list[T]) -> int` |
| `get` | **chiếu theo trường** (khác `.get(i)`) | `get[T](xs: list[T], f: Field) -> list[...]` — **thuộc nhóm bản ghi** |
| `sort_by` | `list.sort_by` mở rộng | `sort_by[T,K](xs: list[T], f: fn(T) -> K, --desc: bool) -> list[T]`, `K ∈ {int,size,duration,str}` |

---

## 4. Quyết định 4 — Bảng task **ST-37..ST-43** (vòng 6, mỗi task ≤ 3 h)

Ghi vào `viscript/agent/QUEUE.md` dưới tiêu đề "Vòng 6 — VSH1a". Phụ thuộc cứng: **ST-35 DONE** và
**ST-36 (R5) DONE** trước ST-37 (ST-35 đụng `tyck`/codegen — cùng vùng với ST-37; ST-34 đã DONE `4ababc6`).
Nếu ST-35 dựng sẵn cơ chế instantiation dùng lại được thì ST-37 **tái dùng**, không viết bộ thứ hai.

| ID | Task | Phụ thuộc | Ngân sách | Tiêu chí xong (đo được) | Write scope |
|---|---|---|---|---|---|
| ST-37 | `CmdSig` **generic** + khe ống **hợp nhất** (không `==`); `take`/`lines` thành generic; seam `SigSource` | ST-36 | 3 h | `echo … \| lines \| take 1` giữ nguyên; `( [1,2] ) \| take 1` xanh; ca âm `take "x"` ⇒ `VS0602`; 338 ca sem **không đổi**; differential 100k **0 lệch** | `vis-types/src/lower/cmd.rs`, `tyck/` |
| ST-38 | Hạ **closure-arg** (`CmdClosure` → `HExpr::Lambda`, cộng offset span) + `where`/`each`/`first`/`count` | ST-37 | 3 h | ≥ 8 fixture `test/sem/vish/` dương + 4 âm (`where 1`, thân trả `int` ⇒ `VS0412`, sai số tham số); lỗi trong thân closure báo **đúng** `dòng:cột`; differential 0 lệch | `lower/cmd.rs`, `parse/cmd.rs` (chỉ đọc), `test/sem/vish/` |
| ST-39 | `sort_by` + `--desc`: mở `list.sort_by` cho khoá `size/duration/str`, giữ ổn định | ST-38 | 3 h | ≥ 6 ca (int/size/str, `--desc`, ổn định khi bằng khoá); ca âm khoá `bool` ⇒ `VS0412`; ca cũ của `list.sort_by` **vẫn xanh** | `vis-interp/src/stdlib.rs`, `lower/cmd.rs` |
| ST-40a | **Prelude bản ghi** `FileEntry`/`ProcEntry` trong `TypeTable` + `it` + **viết tắt trường** (luật dừng trước `--flag`), mã `VS0605` "không có trường" | ST-39 | 3 h | `where size > 5mb` trên `list[FileEntry]` xanh; 6 ca mơ hồ (trước cờ, có ngoặc, `it.` tường minh, trường không tồn tại, `5mb` sai đơn vị, tên trường trùng từ khoá) | `vis-types/src/ty.rs`, `lower/cmd.rs`, `vis-diag` (mã mới) |
| ST-40b | `get`/`select` (chiếu theo trường): hạ thành `map` trên `it.<field>` | ST-40a | 2 h | `… \| get name` ⇒ `list[str]`; ca âm trường không có ⇒ `VS0605`; 3 ca | `lower/cmd.rs`, `test/sem/vish/` |
| ST-41 | **`Render`**: bảng `list[bản ghi]`, khối khoá–giá trị; cột theo **Unicode scalar** sau **NFC** | ST-40b | 3 h | 15 snapshot (Việt NFC/NFD, emoji, CJK, cột rỗng, bảng 1 hàng); bảng **không** phụ thuộc terminal; `print` cũ **không đổi** cho list[str] | `vis-interp/src/builtin.rs` (+`render.rs` mới) |
| ST-42 | `ls`/`open` qua `FsHost` (`list_entries` có metadata) + `--all/--long` + cờ **`--fs-mock`** cho test tất định | ST-41 | 3 h | ≥ 12 ca tất định qua `--fs-mock` (không đọc đĩa thật); `ls ./photos \| where size > 5mb \| sort_by modified --desc \| take 10` **chạy được**; allowlist `doc-examples` **8 → 7** (mục dòng 672 xanh); `ls` không gọi OS từ `vis-interp` | `vis-cli/src/ngoai.rs`, `lower/cmd.rs`, `docs/viscript/doc-examples-allow.txt` (Agent 0) |
| ST-43 | Báo cáo **R6** (VSH1a) | ST-42 | 1,5 h | MD + JSON có `project: "viscript"`, số liệu tái lập, bảng 7 task | `viscript/agent/reports/` |

Tổng ≈ 21,5 h ≈ 3 vòng. `ps`/`kill` **không** có trong bảng: xem §6.

---

## 5. Quyết định 5 — VD cần ghi trước khi Agent 2 cài (câu chữ mẫu)

Số VD bắt đầu từ **VD-68** (VD-67 = ST-34, `4ababc6`).

- **VD-68** *Chữ ký lệnh generic (ST-37).* `CmdSig` mang tham số kiểu `[T]`; khe ống dùng **hợp nhất**
  (`unify`) thay so sánh bằng; xoá kiểu ở biên lệnh theo ST-28; **không** bump `VBC1`; `SigSource`
  (`Builtin | FileCmd | ModuleCmd`) để sau này chuyển verb sang module ViScript mà không đổi `.vish`.
- **VD-69** *Closure-arg (ST-38).* `closure_arg` hạ **trong `cmd_lower`** bằng re-parse span (parser
  không giữ cây con — §A.7); thân thành `HExpr::Lambda`; kiểu `fn(T) -> R`; **cộng offset** span gốc để
  chẩn đoán trỏ đúng `dòng:cột`; `it` **chưa** có ở bước này (xem VD-72).
- **VD-70** *Ngữ nghĩa verb shell (ST-38).* `where` ≡ `.filter`, `each` ≡ `.each`, `first` ≡ `.first`,
  `count` ≡ `.len` — **một** ngữ nghĩa, hai tên; `get`/`select` là **chiếu theo trường** (khác `.get(i)`),
  để nhóm bản ghi; `take`/`lines` thành generic; vòng 6 làm trên **`list`** (stream để VSH1b).
- **VD-71** *`sort_by` (ST-39).* Khoá `K ∈ {int, size, duration, str}`; sắp xếp **ổn định**; `--desc` đảo
  chiều; khoá khác ⇒ `VS0412` kèm gợi ý; mở rộng `list.sort_by` **không** đổi hành vi cũ (ca cũ phải
  xanh); `bool`/`path` **chưa** sắp được ở vòng 6.
- **VD-72** *`it` + viết tắt trường (ST-40a).* Chỉ trong tham số closure của `cmd`; dải token dừng ở
  `--flag`/`|`/`;`/`&` độ sâu 0; ident trần tra `RecDef` ⇒ `it.<field>`; `it` tường minh vẫn hợp lệ;
  trường không tồn tại ⇒ **`VS0605`** (mã mới, sổ `06xx` 4 → 5 mã); không có "biến trần" trong shell
  ⇒ không nhập nhằng với `$x`.
- **VD-73** *Prelude bản ghi (ST-40a).* `FileEntry{name: str, path: path, size: size, modified: int,
  is_dir: bool}` — `modified` là **ms** ở vòng 6, `datetime` để VS2 (`Type` chưa có `datetime`);
  `ProcEntry{pid: int, ppid: int, name: str, cpu: percent, mem: size, state: ProcState}` — **portable**,
  không khái niệm kernel; `Host::procs` vào hợp đồng **chỉ khi** có backend (không làm ở vòng 6);
  `HOST_API` bắt đầu từ 1.
- **VD-74** *`Render` (ST-41).* `list[bản ghi]` ⇒ bảng (tiêu đề = tên trường, canh trái, một dấu cách
  phân cột); bản ghi đơn ⇒ khối `khoá: giá trị`; độ rộng đo theo **scalar** sau **NFC** (NFD tiếng Việt
  làm lệch cột); **không** màu, **không** cắt theo terminal ở vòng 6 (VSH1b); `list[str]` vẫn in như cũ.
- **VD-75** *`ls`/`open` (ST-42).* `FsHost::list_entries(dir) -> Result[list[FileEntry], str]` (một lần
  gọi, có metadata); `--all` gồm mục ẩn, `--long` **không đổi kiểu** (chỉ đổi hiển thị); thứ tự **đã
  sắp** theo tên; lỗi OS ⇒ `Err(str)`; `--fs-mock` là cờ **chỉ để test**, không phải API.

## 6. Quyết định 6 — DoD vòng 6 và những gì **không** làm

**DoD (đo được):**

1. Pipeline §9.7 dòng 672 chạy được thật: `ls ./photos | where size > 5mb | sort_by modified --desc | take 10`.
2. Cổng `doc-examples`: allowlist **8 → 7** (mục 672 xanh). Mục 1451 **vẫn đỏ** (cần `walk`, `to json`,
   `save`) — không hứa xoá.
3. ≥ 26 fixture mới trong `test/sem/vish/` (dương + âm), mỗi lỗi đúng **mã**.
4. `./vis-dev gate --fast` xanh, **không retry**, không nới ngưỡng; differential 100k **0 lệch**;
   482 test hiện có **không mất ca**.
5. `ls`/`ps`-shape test bằng `--fs-mock` (tất định, không đọc đĩa thật).

**Không làm ở vòng 6 (ghi rõ trong QUEUE để khỏi trôi):**

- `stream[T]` lười, SIGPIPE, huỷ pipeline — VSH1b (design §9.5). Vòng 6 **list**; ghi một câu trong VD-70.
- `ps`/`kill` chạy thật — dòng OS (`vis-host-vifast` + `procsrv`), sau VS2. Interface đã chốt ở VD-73.
- `walk`/`save`/`to json`/`from json[T]`/`http` — VSH1b (mục 1451 của allowlist).
- `std.shell` **viết bằng ViScript** — hoãn tới sau khi tách repo (seam `SigSource` giữ đường).
- `cap[Dir]`/`cap[Proc]` **cưỡng chế** — VS2; vòng 6 chỉ đi qua `Host` + log `--explain-perms`.

---

## 7. Bản nháp trả lời issue (dán được — giọng maintainer)

> Cảm ơn bạn, phần "hiện trạng" bạn khảo sát **đúng** gần hết (3 builtin, thiếu `where/sort_by`, thiếu
> `Render`, thiếu bản ghi). Ba chỗ mình chỉnh lại:
>
> 1. **Head đã đi tiếp**: ST-33 (`json.decode` + `std.fs` qua `Host`, `f9e6db5`) và ST-34 (`| complete` +
>    pipe→stdin, `4ababc6`) vừa xong — **482 test**; P3 **không còn chờ ST-33**.
> 2. **P1 không thuần**: khe ống hiện khớp kiểu **bằng đúng**, nên `… | where … | take 10` chết ngay ở
>    `take`; muốn nối được phải làm **chữ ký lệnh generic** trước. Thêm nữa `it` + viết tắt trường phải đi
>    cùng **bản ghi** (và bản ghi phải có schema), nên mình gộp lại thành nhóm.
> 3. **P4 không thuộc ViScript**: `ProcEntry`/`Host::procs` là **hợp đồng** của ViScript, nhưng `ps`/`kill`
>    chạy thật là `vis-host-vifast` + RPC `procsrv` — nằm ngoài `viscript/` và sau VS2.
>
> Về lộ trình: mình nhận hướng này và đặt nó thành **VSH1a** (vòng 6, sau R5) — đúng là nửa đầu của mốc
> VSH1 trong §19, không phải việc chen ngang. Ngoài ra ViScript sẽ **tách thành repo riêng**, nên từ giờ
> mọi thứ thuộc ngôn ngữ (kiểu, `Render`, prelude bản ghi, `Host`) nằm trong `viscript/`, backend OS nằm ngoài.
>
> Trả lời 4 câu: (1) hợp lộ trình, nhưng vào **vòng 6 sau ST-36**, và phải thành task trong `QUEUE.md` +
> VD do Agent 0 duyệt (thay đổi ngữ nghĩa: Khối A cấm #6) — bạn đừng mở PR trước khi có mã task; (2) theo
> **§9.7** — §9.7 và §13 không xung đột, cùng `snake_case`; `first` chưa có trong hai bảng nên mình đưa vào
> cùng đợt và ghi VD; (3) **có**, cần VD cho viết tắt trường (luật dừng trước `--flag`, mã "không có
> trường"); (4) **không trùng** ST-34 về chức năng, nhưng **chạm cùng tệp** (`lower/cmd.rs`) nên làm sau.
>
> Việc bạn có thể làm ngay mà không cần chờ: (a) soạn **bộ golden ca mơ hồ** cho `where size > 5mb` /
> `sort_by modified --desc` / `get name` (dạng `.vish` + kỳ vọng, chưa cần code chạy); (b) phác **snapshot
> bảng** cho ST-41 (tiếng Việt NFC/NFD, emoji); (c) rà `list.sort_by` xem khoá `size/str` cần gì. Mở PR
> khi task tương ứng đã có trong QUEUE.

---

## 8. Agent 0 cần làm (~15 phút)

1. Ghi **VD-68..VD-75** vào `viscript/agent/DECISIONS.md` (câu chữ ở §5).
2. Thêm khối "Vòng 6 — VSH1a" + 8 hàng **ST-37..ST-43** (ST-40 tách a/b) vào `viscript/agent/QUEUE.md` (§4),
   trạng thái `ST-37 = READY` (sau khi ST-36 DONE), còn lại `BLOCKED`.
3. Thêm 1 dòng vào `docs/viscript/doc-examples-allow.txt` chú thích "mục 672 sẽ xanh ở ST-42" — hoặc
   để nguyên, chỉ cần **không** xoá mục 672 trước khi ST-42 xanh (cổng sẽ đỏ nếu khai sai).
4. Gắn nhãn `viscript` + `vsh1a` cho issue #21; ghi chú trong issue: "sẽ chuyển sang repo ViScript khi tách".
5. **Không** sửa `docs/viscript/ViScript-thiet-ke.md` (đóng băng) — schema/prelude/`Render` đi vào VD.

**Nếu không đồng ý đề xuất:**

- (i) Muốn ra hình thái nhanh hơn nữa: bỏ ST-40a/40b/41 khỏi vòng 6, chỉ làm ST-37..39 (2 vòng) — nhưng
  khi đó §9.7 vẫn đỏ và không có bảng, tức "hình thái cơ bản" chỉ đúng cho `list[str]`.
- (ii) Muốn theo đúng chữ issue (P1 riêng, P3 ngay): chấp nhận `take` không nối được và `ls` trả `list[str]`
  — sẽ phải làm lại khi có bản ghi; **không** khuyến nghị.
- (iii) Muốn `std.shell` viết bằng ViScript ngay: đổi ST-37 thành "hạ tầng module nhúng + `cmd` trong
  module" (3 h → 6 h), đẩy ST-42 lùi một vòng.

---

## 9. Phụ lục — Bằng chứng đo được hôm nay (HEAD `0e65446`, 482 test)

```
$ vis check a.vish   # ls ./photos | where size > 5mb | sort_by modified --desc | take 10
not ok 1 - a.vish
# VS0603 2:60 pipeline không nối được: `take` không có khe nhận `error`

$ vis check b.vish   # ps | where cpu > 50pct
ok 1                                  ← xanh giả: tên lạ = chương trình ngoài
$ vis run b.vish
b.vish:2:6: error VS0501: không chạy được chương trình ngoài `where`: No such file or directory

$ vis check c.vish   # echo "a\nb" | lines | take 1
ok 1
$ vis run c.vish
[a]                                   ← Show của list, không có bảng
```

Sự thật code dùng trong tệp này: `take`/`lines` đơn hình (`lower/cmd.rs:80`), khe ống `==`
(`lower/cmd.rs:376`), `CmdClosure` không có nhánh hạ (grep 0 trong `vis-types`), `filter/each` đã kiểm
kiểu (`tyck/expr.rs:767`), `list.sort_by` khoá `int` + ổn định (`vis-interp/src/stdlib.rs:228`),
`std.*` khai `Ty::Any` có chủ ý (VD-59 (3)), allowlist `doc-examples` **8 mục** (dòng 672 = §9.7,
1451 = phụ lục B), 11 tệp test, 18 chỗ đọc `../../../docs/viscript/…` (nợ tách repo).
