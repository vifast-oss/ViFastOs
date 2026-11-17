<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# KT-109 — 22 lệnh `deferred-K7` của `spec/v1-parity.toml`: vì sao hoãn · nhóm · công · rủi ro · thứ tự cho K7

- **Task**: KT-109 (Agent 2 dòng OS). **Ngân sách**: 2 h.
- **Chế độ**: **chỉ đọc** + **chỉ ghi** trong `v2/agent/JOURNAL/kt109-*`. **Không** sửa `spec/`,
  **không** sửa mã, **không** commit, **không** `git add`.
- **Câu hỏi**: 22 mục `deferred` hoãn vì cái gì **thật**, phân nhóm dứt khoát, ước lượng công +
  rủi ro **có căn cứ**, và xếp thứ tự làm cho K7.

## §0. Cách làm

1. Đọc `spec/v1-parity.toml` (396 dòng) và **đếm bằng máy** (script `python3` tách `[[command]]`).
2. Với **từng** lệnh: đọc `reason` trong spec, rồi **kiểm lại bằng mã** — không tin `reason`
   (review KT-77a đã chứng minh 3 `reason` sai hiện trạng: `time`, `uptime`, `jobs`).
3. Tra chéo: `v2/agent/DECISIONS.md` (D2-117, D2-119, D2-121, D2-123, D2-125, D2-127),
   `v2/agent/QUEUE.md`, `v2/agent/BLOCKED.md`, `v2/agent/reports/` (`K6-DOD.md`,
   `QUYET-DINH-A0-K6-2026-11-17.md`, `BRIEFING-Agent0-K6.md`), `v2/docs/` (`k6-parity.md`,
   `k6-exec.md`, `k6-product-fs.md`, `abi-v1-v2-mapping.md`), `v2/agent/JOURNAL/` (`kt77a`,
   `kt77c`, `kt104`, `kt105`).
4. Đọc mã: `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs` (685 dòng),
   `src/proc_fs.rs` (610), `src/proc_abi.rs`, `src/proc_rt.rs`, `init/procsrv_user.rs`,
   `crates/vifast-fs/src/lib.rs` (4543), `v2/kernel/vifast-core/src/{machine,arena}.rs`,
   `v2/kernel/vifast-plat-riscv64/src/{user,init_pages}.rs`, `v2/test/boot/ktest.sh`,
   `v2/test/parity/run.sh`.
5. Đọc bằng chứng v1: `docs/v1/overnight-log.md` (đọc-only) và **tag `v1-archive`** qua
   `git show` (chỉ đọc, không ghi).

## §1. Đếm: **đúng 22** — và khớp hai chiều

Đếm bằng máy trên `spec/v1-parity.toml`: **28** mục = **6** `active` + **22** `deferred-K7`.

- `active` (6): `help`, `echo`, `ps`, `kill`, `ls`, `cat`.
- `deferred` (22): `cd`, `pwd`, `time`, `mkdir`, `rm`, `mv`, `ln`, `stat`, `sync`, `truncate`,
  `fsck`, `free`, `df`, `uptime`, `uname`, `run`, `runfs`, `exec`, `jobs`, `loop`, `history`, `!!`.

**Kiểm chéo độc lập** (không dựa vào con số 22 của spec): bảng sinh tự động ở tag
`v1-archive:docs/11-hien-tai/16-van-hanh.md` mục `SHELL-TABLE` có **22 builtin**:
`!! cat cd echo exec fsck help history kill ln loop ls mkdir mv pwd rm run runfs stat sync time truncate`.
Trong đó **5** đã `active` (`help`, `echo`, `kill`, `ls`, `cat`) ⇒ **17** builtin còn hoãn.
Cộng **6 applet ngoài** (`ps free df uname uptime jobs`), trong đó **1** đã `active` (`ps`) ⇒ **5**
applet hoãn. **17 + 5 = 22.** ✔ Khớp tuyệt đối, không phải ép số.

> Ghi chú: `ps` là **applet** ở v1 (`/bin/ps`), không nằm trong 22 builtin — nên nó không làm lệch
> phép đếm trên.

## §2. Bảng 22 lệnh

Cột **nhóm** = **chướng ngại vật chất** phải xây (xem định nghĩa ngay dưới bảng).
Cột **chờ A0?** = có cần **quyết định** của Agent 0 **trước khi bắt tay** hay không (khác với hai
cổng thủ tục chung ở §3, áp cho **cả 22**).

| # | mã | args | vì sao hoãn (bằng chứng) | nhóm | công (h) | rủi ro | chờ A0? |
|---|---|---|---|---|---|---|---|
| 1 | `cd` | `<path>` | `spec/v1-parity.toml:132` ghi *hai dữ kiện mâu thuẫn*. **Kiểm lại ở tag**: mã có comment `r249 (T13)` và `r299` (`v1-archive:crates/vifast-client/src/main.rs:877-890`) ⇒ `cd`/`pwd` được **đưa lại sau** hai vòng revert 222/223 ⇒ mâu thuẫn **giải được**. Nhưng spec vẫn `decision='cho Agent 0'`, và shell v2 **không có** trạng thái cwd | C | 0,75 | vừa | **Có** — chốt "có ở v1" + quyền sửa spec |
| 2 | `pwd` | — | như `cd` (cùng cặp, cùng một quyết định, cùng một ô `cwd`) | C | 0,75 | vừa | **Có** |
| 3 | `time` | — | thiếu **builtin**; `ClockRead` = syscall **11** đã có (`spec/abi.toml:117`, `v2/kernel/vifast-plat-riscv64/src/user.rs:1229`, TAP `ktest_user.rs:1105`) | A | 0,25 | thấp | Không |
| 4 | `mkdir` | `<path>` | `Fs::mkdir` **có** (`crates/vifast-fs/src/lib.rs:1962`) nhưng **dịch vụ FS không có opcode**: `proc_fs.rs:41-47` chỉ có 7 opcode `PATH/LS/CAT/WRITE/TRUNC/STAT/SYNC` | B | 0,5 | vừa | Không |
| 5 | `rm` | `<path>` | `Fs::unlink` **có** (`lib.rs:2626`), thiếu opcode | B | 0,5 | vừa | Không |
| 6 | `mv` | `<src> <dst>` | `Fs::rename` **có** (`lib.rs:2657`), thiếu opcode | B | 0,5 | vừa | Không |
| 7 | `ln` | `<src> <dst>` | `Fs::link` **có** (`lib.rs:2591`), thiếu opcode | B | 0,5 | vừa | Không |
| 8 | `stat` | `<path>` | **`OP_STAT` ĐÃ CÓ** (`proc_abi.rs:98`, handler `proc_fs.rs:582` trả `kind<<8` + `size` + `ino`) nhưng shell **chưa gọi** (`grep -c OP_STAT shell_user.rs` = **0**) ⇒ **chỉ thiếu builtin**. `reason` trong spec ("shell v2 chua co lenh FS") **đã cũ** sau KT-77c | A | 0,25 | thấp | Không |
| 9 | `sync` | — | **`OP_SYNC` ĐÃ CÓ** và `write` **đang dùng** (`shell_user.rs:547`) ⇒ chỉ thiếu builtin. `reason` trong spec ("v2 chua co fsync tu user mode") **sai tầng**: nợ đó là của **ảnh persist** (D2-117), không phải dịch vụ FS sản phẩm | A | 0,25 | thấp | Không |
| 10 | `truncate` | `<path> <size>` | **`OP_TRUNC` ĐÃ CÓ** và `write` **đang dùng** (`shell_user.rs:535`) ⇒ chỉ thiếu builtin | A | 0,25 | thấp | Không |
| 11 | `fsck` | — | `Fs::fsck` **có** (`lib.rs:1021`), thiếu opcode | B | 0,5 | vừa | Không |
| 12 | `free` | — | v1 dùng syscall `MemInfo` = **53** (`docs/v1/overnight-log.md:2638-2640`); v2 **bỏ** syscall này (`v2/docs/abi-v1-v2-mapping.md:108`: "Untyped watermark nội bộ"). Kernel **có** số (`machine.rs:708` `total_used`, `:683` `untyped_free`, `arena.rs:89` `free`) nhưng **không có dịch vụ** trả ra U-mode | B | 1,0 | vừa | Không (thành **C** nếu A0 chọn mở syscall `MemInfo`) |
| 13 | `df` | — | v1 dùng `vfs::STATFS` (`v1-archive:docs/11-hien-tai/16-van-hanh.md:95`); `Fs::usage()` **có** (`lib.rs:2025`, trả `(used, total)` block vùng DATA) nhưng thiếu opcode | B | 0,5 | vừa | Không |
| 14 | `uptime` | — | thiếu **builtin**; `ClockRead` đã có; ở v1 là **applet** `/bin/uptime` (`v1-archive:crates/vifast-bin/src/main.rs:224`) | A | 0,25 | thấp | Không |
| 15 | `uname` | — | thiếu builtin **và** chuỗi v1 `ViFastOs <arch> bootinfo-v3` (`crates/vifast-bin/src/main.rs:80`) **không tái lập trung thực được**: v2 **không có** giao thức bootinfo (grep toàn repo: **0** hit ngoài `crates/` v1 + chính spec) ⇒ phải **định nghĩa lại kỳ vọng** (ứng viên: `DebugGetInfo` = syscall 15 trả `abi_version`/`num_harts`, `spec/abi.toml:120`) | D | 0,5 | thấp | **Có** — chốt chuỗi thay cho `bootinfo-v3` |
| 16 | `run` | `<path>` | v1: root `spawn_elf_from_fs` (`docs/v1/overnight-log.md:1582`). v2 **không có đường ABI** cho U-mode yêu cầu exec/spawn: `v2/docs/k6-exec.md:104-108` ("ABI hiện tại **không** có đường cho U-mode yêu cầu `exec`"), `:255` (nợ `Tcb.Exec(path)`), `v2/agent/BLOCKED.md:378`, `v2/agent/reports/K6-DOD.md:466` (P10) | C | 1,5 | cao | **Có** — mở `Tcb.Exec(path)` |
| 17 | `runfs` | `<path> [args...]` | như `run`, thêm truyền tham số (`HIARG`) | C | 1,5 | cao | **Có** |
| 18 | `exec` | `<path> [args...]` | như `run`; nợ đích danh `Tcb.Exec(path)`; **OD-K6-8** nói "Tcb.Exec(path) **nếu v1-parity cần**" ⇒ **cần** (3 mục này) | C | 1,0 | cao | **Có** |
| 19 | `jobs` | — | thiếu **builtin**; dữ liệu state từng TCB **đã có** trong reply `PS_REQ` (`shell_user.rs:235-268` tự giải `pid=entry>>5`, `state=(entry>>1)&0x0F`) ⇒ chỉ cần đếm `Ready`/`Running` | A | 0,25 | thấp | Không |
| 20 | `loop` | `<n>` | v1: client gửi `LABEL_LOOP`, **root** lặp spawn→destroy `n` lần (`docs/v1/overnight-log.md:3388-3396`). v2 **không có** dịch vụ spawn/destroy: procsrv chỉ có `WAIT`/`KILL` (`init/procsrv_user.rs:36-39`), ảnh sản phẩm nạp **cố định** procsrv+shell+victim (`src/proc_rt.rs:75-78`), `TCB_SLOTS` chỉ **5** slot (`src/proc_abi.rs:49`) | B | 2,5 | **cao** | Không |
| 21 | `history` | — | shell v2 **không có** lịch sử lệnh; v1 dùng ô **32 B** (`docs/v1/overnight-log.md:2776-2777`) | A | 0,25 | thấp | Không |
| 22 | `!!` | — | như `history` (cùng ô nhớ, `!!` chạy lại dòng trước) | A | 0,25 | thấp | Không |

### Định nghĩa nhóm (dùng cho bảng trên)

| Nhóm | Nghĩa | Hệ quả cho K7 |
|---|---|---|
| **A** | **Chỉ** thiếu builtin trong shell; mọi thứ kernel/dịch vụ/syscall **đã có và gọi được hôm nay** | Việc thuần `shell_user.rs` + `spec` |
| **B** | Thiếu builtin **và** thiếu năng lực kernel/dịch vụ; **không** đổi ABI (giao thức dịch vụ **nội bộ**: opcode FS, label dịch vụ PS) | Chạm `proc_fs.rs`/`proc_rt.rs` — vẫn trong tầm một task K7 |
| **C** | Cần **đổi ABI** (`spec/abi.toml`) hoặc cần **quyết định ghi trong `spec/`** | **Phải qua Agent 0** trước khi viết mã |
| **D** | Cần **đổi/định nghĩa lại kỳ vọng** (không thể đạt kỳ vọng v1 một cách trung thực) | **Phải qua Agent 0** |
| **E** | Khác | — |

**Đếm theo nhóm: A = 8 · B = 8 · C = 5 · D = 1 · E = 0 → 22.** ✔

## §3. Hai cổng **chung** — không phải thuộc tính của từng lệnh

Hai việc dưới đây **chặn cả 22 lệnh** nhưng **không** làm lệch phép đếm ở §2 (nếu cộng dồn vào
từng lệnh thì tổng sẽ thành 44 — vô nghĩa). Ghi riêng để lead không bỏ sót.

### G1 — quyền sửa `spec/v1-parity.toml` (kiểu **C**, thủ tục)

Mỗi lệnh muốn thành `active` phải: (i) đổi `status`, (ii) viết `expect` thành **regex** (hiện là
**văn xuôi**, ví dụ `expect = 'tao thu muc, khong in loi'`), (iii) có thể chỉnh `serial_min_count`.
`spec/` bị **cấm tuyệt đối** (AGENTS.md §4.1) trừ task được ghi rõ. Tiền lệ đúng việc này:
**D2-123.2** cho KT-77c chuyển `ls`/`cat` từ `deferred` sang `active` trong `v1-parity.toml`.
⇒ K7 **phải** có một task được A0 ghi rõ quyền sửa `v1-parity.toml`; không có nó thì **không lệnh
nào** chuyển được trạng thái dù mã đã xong.

### G2 — kỳ vọng harness `cmds=` (kiểu **D**)

Ba chỗ **hard-code** số lệnh của shell:

| Tệp:dòng | Nội dung |
|---|---|
| `v2/test/boot/ktest.sh:708` | `^# shell: up cmds=help/echo/ps/wait/kill/exit$` |
| `v2/test/boot/ktest.sh:709` | `^# shell: help cmds=6$` |
| `v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:761` | `captured_contains(SLOT_SHELL, b"# shell: help cmds=6")` |

Thêm builtin vào **ảnh test** ⇒ cả ba đỏ. **KT-77c đã né đúng** bằng `#[cfg(shell_fs)]`: shell ảnh
test giữ **6** (`shell_user.rs:86-91`, comment ghi rõ *"kỳ vọng `# shell: help cmds=6` của harness
… **không** phải sửa"*). Làm y hệt cho 22 lệnh ⇒ **không** phải sửa kỳ vọng cũ, **nhưng** khi đó
`--mode transcript` (chạy trên ảnh test) **không phủ** được lệnh mới ⇒ phải thêm một trường kiểu
`image = 'product'` cho mục trong spec, hoặc cho runner bỏ qua theo ảnh. **Cả hai đường đều là
quyết định A0** (D2-121(c): Agent 1 **không** được đổi kỳ vọng ca cũ). Công ~0,5–1 h, làm **một lần**.

## §4. Thứ tự đề nghị

**Nguyên tắc xếp**: OD-K6-7 định nghĩa v2.0 hoàn thiện khi *"chạy hết kịch bản
`spec/v1-parity.toml` (**không còn mục `deferred`**)"* ⇒ **giá trị của 22 lệnh bằng nhau** (đều là
điều kiện cần của v2.0). Vì vậy tối ưu là **rẻ trước**, và trong cùng độ rẻ thì **rủi ro thấp
trước** (D2-03: hồi quy thì revert trước, điều tra sau).

| Đợt | Nội dung | Lệnh | Công | Vì sao đợt này |
|---|---|---|---|---|
| **0** | Hai cổng A0: **G1** (quyền sửa `v1-parity.toml`) + **G2** (`cmds=`/transcript) | — | 0,5–1,0 h (chờ) | Không có G1/G2 thì **không lệnh nào** đóng được; làm song song với đợt 1 được |
| **1** | **A — builtin thuần** (2,0 h, 8 lệnh) | `sync` → `stat` → `truncate` → `time` → `uptime` → `jobs` → `history` → `!!` | 2,0 | Opcode/syscall **đã có sẵn**; mỗi lệnh ~15 phút; đóng 8/22 = 36 % số mục |
| **2** | **B rẻ — opcode FS** (2,0 h, 6 lệnh) | `df` → `fsck` → `mkdir` → `rm` → `mv` → `ln` | 2,0 | Hàm FS **đã có** (`usage`/`fsck`/`mkdir`/`unlink`/`rename`/`link`), chỉ thiếu đường dịch vụ; `df`/`fsck` trước vì **1 opcode/lệnh** và giá trị quan sát cao (sức khoẻ đĩa) |
| **3** | **C — sau quyết định A0** (5,5 h, 5 lệnh) | `cd`+`pwd` (1,5) → `exec` (1,0) → `run` (1,5) → `runfs` (1,5) | 5,5 | `exec` trước vì nó **là cơ chế**; `run`/`runfs` dùng lại. `cd`/`pwd` rẻ nhưng phải chờ A0 chốt sự tồn tại |
| **4** | **B đắt — rủi ro cao** (3,5 h, 2 lệnh) | `free` (1,0) → `loop` (2,5) | 3,5 | `loop` cần dịch vụ spawn/destroy trong **ảnh sản phẩm** ⇒ phải soak lại; để cuối để không chặn 20 lệnh kia |
| **5** | **D** (0,5 h, 1 lệnh) | `uname` | 0,5 | Rẻ nhưng **chờ A0 chốt chuỗi**; nếu A0 chốt sớm thì **kéo lên đợt 1** |

**Năm lệnh nên làm trước** (đợt 1, rẻ nhất — mỗi lệnh ~15 phút, **0** dòng kernel):

1. **`sync`** — `OP_SYNC` đã có, `write` đang gọi (`shell_user.rs:547`); builtin = 1 `fs_call` + in.
2. **`stat`** — `OP_STAT` đã có trong dịch vụ FS (`proc_fs.rs:582`) nhưng **chưa ai gọi**; builtin
   chỉ cần đọc `kind`/`size` từ reply.
3. **`truncate`** — `OP_TRUNC` đã có, `write` đang gọi (`shell_user.rs:535`).
4. **`time`** — `ClockRead` (11) đã có + đã có TAP; `uptime` đi kèm **cùng** một lần sửa (cùng
   syscall) nên thực chất đợt này đóng **2** lệnh.
5. **`jobs`** — dữ liệu đã nằm trong reply `PS_REQ` mà `ps` đang dùng; chỉ cần đếm `Ready`/`Running`.

> Nếu A0 chốt G2 muộn, đợt 1 vẫn **viết được mã** (builtin + `#[cfg(shell_fs)]`) và chỉ **chờ**
> bước flip `status` — nên đợt 1 không bị chặn cứng.

## §5. Tổng công

| Nhóm | Số lệnh | Công |
|---|---|---|
| A | 8 | 2,0 h |
| B | 8 | 6,5 h |
| C | 5 | 5,5 h |
| D | 1 | 0,5 h |
| **Tổng 22 lệnh** | **22** | **14,5 h** |
| Cổng chung G1 + G2 (một lần) | — | 0,5–1,0 h |
| **Tổng K7 (ước lượng)** | | **≈ 15,5 h** (khoảng 14–19 h) |

**Căn cứ ước lượng** (không đoán bừa):

- **KT-77c** (đo được, `JOURNAL/kt77c-journal.md:4`): **~1 h 05 m** cho **7 opcode FS + 3 builtin**
  (`ls`/`cat`/`write`) + tách feature ảnh sản phẩm + 2 boot + parity 6/6 + 3 ca âm + tài liệu
  ⇒ **~20 phút/builtin khi dịch vụ đã có**. Đây là mốc cho nhóm **A** (0,25 h/lệnh, có dư cho
  `expect` + ca âm + chạy parity).
- **KT-104** (`kt104-journal.md:3`): sửa nhỏ trong shell = **~10 phút**. **KT-105**
  (`kt105-journal.md:3`): sửa lỗi thật trong shell = **~20 phút**. ⇒ 0,25 h/lệnh nhóm A là **sát**,
  không lạc quan.
- Nhóm **B**: mỗi opcode mới = handler trong `proc_fs.rs` (~25–40 dòng, theo khuôn `stat`/`sync`
  sẵn có) + builtin + ca âm ⇒ **0,5 h/lệnh**; 4 lệnh `mkdir`/`rm`/`mv`/`ln` dùng **chung** đường
  `fs_set_path` nên không nhân chi phí.
- Nhóm **C**: `run`/`runfs`/`exec` **chia sẻ một cơ chế** (exec/spawn từ FS) ⇒ 3,0 h cơ chế +
  1,0 h cho 3 builtin = **4,0 h**; `cd`/`pwd` chia sẻ một ô `cwd` ⇒ **1,5 h** cho cả cặp.
- `loop` **2,5 h** và rủi ro **cao**: cần dịch vụ spawn/destroy trong ảnh **sản phẩm** với chỉ
  **5** `TCB_SLOTS`, và v1 đã gặp đúng bài học cạn bộ nhớ khi spawn
  (`docs/v1/overnight-log.md:1582-1590`: *"DATA untyped gần cạn (85/86 page) … spawn fail
  `map_frame_fallible`"*).
- **Chưa tính** vào 15,5 h: ma trận 24 boot + soak 1.000 trên ảnh cuối (OD-K6-2 tiêu chí 4) — đó là
  task cổng riêng, không phải công của từng lệnh.

## §6. Năm phát hiện làm thay đổi bức tranh

1. **`cd`/`pwd`: mâu thuẫn trong spec GIẢI ĐƯỢC — không cần A0 "đoán".** `reason` của spec dẫn
   `docs/v1/overnight-log.md:3664-3669` (vòng 222 revert) làm phản chứng. Nhưng mã ở **đúng tag**
   `v1-archive` có comment `r249 (T13)` và `r299`
   (`v1-archive:crates/vifast-client/src/main.rs:877-890`) ⇒ tính năng được **đưa lại sau** vòng
   222/223 (vòng 223 cũng revert, xem `overnight-log.md:3676-3686`). Vậy hai dữ kiện **không** mâu
   thuẫn: chúng ở **hai thời điểm khác nhau**, và trạng thái cuối (tag) là **CÓ**. Đề nghị A0 chốt
   "có" — nhưng vẫn phải chốt, vì spec đang ghi `decision='cho Agent 0'`.
2. **5 `reason` trong spec đã cũ hoặc sai tầng** — `stat`, `sync`, `truncate` (opcode **đã có**),
   `df`, `fsck` (hàm FS **đã có**). Cụm `reason` *"shell v2 chua co lenh FS => KT-77b/K7"* được
   chép cho 7 mục (`mkdir`/`rm`/`mv`/`ln`/`stat`/`truncate`/`fsck`) — đúng **một nửa** (thiếu
   builtin) nhưng **sai** ở chỗ ngụ ý cần KT-77b, trong khi **KT-77b đã DONE** và dịch vụ FS đã có
   từ KT-77c. Nếu K7 lập kế hoạch theo `reason` cũ sẽ **ước lượng cao hơn thực tế** cho 5 lệnh.
3. **`sync` không phải nợ D2-117.** D2-117 nói *"chưa `fsync` từ U-mode"* ở **ảnh persist**; dịch vụ
   FS của ảnh **sản phẩm** đã `fsync` qua `OP_SYNC` và `write` **đang dùng** nó
   (`shell_user.rs:547`). `sync` chỉ là builtin gọi lại đúng opcode đó.
4. **`uname` là lệnh duy nhất không thể "trung thực" theo kỳ vọng v1.** Chuỗi `bootinfo-v3` là
   giao thức của v1 (`crates/vifast-bin/src/main.rs:80`); v2 **không có** bootinfo. In nguyên chuỗi
   đó sẽ là **nói sai** — nên phải định nghĩa lại kỳ vọng (ứng viên: `DebugGetInfo` = syscall 15).
   Đây là lý do `uname` xếp nhóm **D** chứ không phải **A**.
5. **`loop` — không phải `run`/`runfs`/`exec` — mới là lệnh đắt và rủi ro nhất.** Nó cần **dịch vụ
   spawn/destroy** trong ảnh **sản phẩm** (procsrv hiện chỉ `WAIT`/`KILL`; ảnh nạp cố định 3 tiến
   trình; chỉ 5 `TCB_SLOTS`), tức chạm đúng chỗ v1 đã từng cạn bộ nhớ. `run`/`runfs`/`exec` đắt vì
   **ABI**, nhưng cơ chế đã có sẵn một nửa (`v2/kernel/vifast-plat-riscv64/src/exec.rs`).

**Bối cảnh phạm vi (quan trọng cho lead)**: **OD-K6-7** định nghĩa v2.0 hoàn thiện khi *"chạy hết
kịch bản `spec/v1-parity.toml` (**không còn mục `deferred`**)"*, và **OD-K6-8** đưa
`Tcb.Exec(path)` vào K7 *"nếu v1-parity cần"* ⇒ **cần**. Vậy **cả 22** nằm trên đường tới v2.0,
không phải "nice to have"; và **5 lệnh nhóm C + 1 nhóm D cần A0 quyết trước** thì K7 mới đóng được
cửa.

## §7. Đã tra ở đâu · KHÔNG TÌM THẤY

**Đã tra** (đọc thật, không suy đoán):

- `spec/v1-parity.toml` (toàn bộ 396 dòng, đếm bằng máy), `spec/abi.toml` (bảng syscall + object).
- `v2/agent/DECISIONS.md`: D2-117, D2-118, D2-119, D2-121, D2-123, D2-124, D2-125, D2-126, D2-127.
- `v2/agent/QUEUE.md` (KT-77a/77b/77c, KT-81/82/83, khối F), `v2/agent/BLOCKED.md` (§KT-77a,
  dòng 378, 451, 481-483), `v2/agent/STATE.md` (dòng 3303).
- `v2/agent/reports/`: `QUYET-DINH-A0-K6-2026-11-17.md` (OD-K6-1..8), `K6-DOD.md` (P10),
  `BRIEFING-Agent0-K6.md`, `A1-decision-log.md`.
- `v2/docs/`: `k6-parity.md`, `k6-exec.md` (§4.1, §8, §9), `k6-product-fs.md`, `k6-product-shell.md`,
  `abi-v1-v2-mapping.md` (§2 bảng 58 hàng), `k6-harness.md`.
- `v2/agent/JOURNAL/`: `kt77a-journal.md`, `kt77c-journal.md`, `kt104-journal.md`, `kt105-journal.md`.
- Mã: `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs`, `src/proc_fs.rs`, `src/proc_abi.rs`,
  `src/proc_rt.rs`, `src/proc_demo.rs`, `init/procsrv_user.rs`, `Cargo.toml`, `build.rs`;
  `crates/vifast-fs/src/lib.rs`; `v2/kernel/vifast-core/src/{machine,arena}.rs`;
  `v2/kernel/vifast-plat-riscv64/src/{user,init_pages,exec}.rs`;
  `v2/test/boot/ktest.sh`, `v2/test/parity/run.sh`.
- v1: `docs/v1/overnight-log.md` (các vòng 89, 141, 154, 201, 218, 222, 223),
  `docs/v1/agent2-report-21.md`, và **tag `v1-archive`** qua `git show` (chỉ đọc):
  `crates/vifast-client/src/main.rs`, `crates/vifast-bin/src/main.rs`,
  `docs/11-hien-tai/16-van-hanh.md` (mục `SHELL-TABLE`).

**KHÔNG TÌM THẤY**: **không có mục nào**. Cả 22 lệnh đều truy được nguyên nhân tới `file:dòng`
(hoặc trong `reason` của spec, hoặc trong mã/dịch vụ v2, hoặc trong log v1). Chỗ **mỏng nhất** là
`loop`: `reason` trong spec chỉ một câu (*"giao thuc loop o root chua co tren v2"*), nên nguyên nhân
đầy đủ phải dựng lại từ `docs/v1/overnight-log.md:3388-3396` (v1 đặt vòng lặp ở **root**) +
`init/procsrv_user.rs:36-39` (chỉ `WAIT`/`KILL`) + `src/proc_rt.rs:75-78` (nạp cố định) +
`src/proc_abi.rs:49` (5 `TCB_SLOTS`) — **đã dựng lại được**, không phải "không tìm thấy".

## §8. Việc chưa làm (ngoài phạm vi KT-109)

- **Không** sửa `spec/v1-parity.toml` (cấm tuyệt đối) — kể cả các `reason` đã cũ ở §6.2. Đề nghị
  lead giao việc đó cho task K7 **được A0 ghi rõ quyền sửa spec** (G1).
- **Không** sửa mã, **không** chạy cổng/parity (task chỉ đọc + viết nhật ký).
- **Không** commit, **không** `git add`.
