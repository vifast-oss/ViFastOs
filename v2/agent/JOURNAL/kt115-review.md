# KT-115 — Review độc lập toàn dòng OS v2

**Ngày**: 2026-11-18 · **Ngân sách**: 3 h · **Dùng**: ~1 h 20 phút.
**Phạm vi**: `v2/kernel/**`, `v2/lib/**`, `v2/sim/**`, `v2/test/**`, `v2/docs/**`, `spec/v1-parity.toml`.
**Ràng buộc đã giữ**: chỉ đọc + ghi trong `v2/agent/JOURNAL/kt115-*`; **không** sửa mã, **không**
`git add`, **không** commit; mọi thí nghiệm trong `/tmp/kt115-repo` (clone `--shared` của HEAD) +
`/tmp/kt115-target`; QEMU tối đa **một** cái một lúc, chỉ `kill <pid>` của mình; **không** nightly,
**không** soak, **không** ma trận đầy đủ.

**HEAD lúc review**: `11550366` (`[KT-114]`). Trong lúc review, HEAD nhảy từ `df44d5d1` → `9ddffa0f`
→ `11550366` (hai agent khác đang chạy song song). **Mọi số dòng dưới đây đọc từ `11550366`**; các
phát hiện đã được **kiểm lại** trên HEAD mới (xem cột bằng chứng). Ảnh dùng để đo:
`v2/artifacts/0b79a652…bin` (canonical KT-113) + ảnh build lại từ `/tmp/kt115-repo` (sha
`152f59aa…` cho `product-fs`, khớp `inputs=f5f7de891e796189`).

**Cách làm**: đọc mã trước, **không** tin báo cáo cũ; mỗi phát hiện cố gắng dựng ca chạy được
(clone `/tmp` + sửa **bản clone** + build + boot QEMU + dán log). Log bằng chứng nằm ở
`v2/agent/JOURNAL/kt115-logs/`.

---

## 1. Bảng phát hiện

| # | Phát hiện | file:dòng | Mức độ | Bằng chứng | Đề nghị vá |
|---|---|---|---|---|---|
| 1 | **`load_path` đọc quá biên `words[4]` ⇒ PANIC kernel từ dữ liệu U-mode.** `len` lấy từ `head>>40` (8 bit, ≤255) rồi chỉ kẹp theo `MAX_PATH` (48) **không** kẹp theo số byte thật trong 3 MR word (24). `len ≥ 25` ⇒ `words[1 + index/8]` với `index=24` ⇒ `words[4]` trên `[u64; 4]`. | `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs:450,454` | **nặng** | `kt115-logs/kt115-oob-panic.log`: sửa **bản clone** cho shell gửi `OP_PATH \| 25<<40` ⇒ boot in `PANIC at vifast-kbin-riscv64/src/proc_fs.rs:454:24` / `PANIC: index out of bounds: the len is 4 but the index is 4`; máy chết, shell không trả lời. Tái hiện trên HEAD `11550366`. | `let len = core::cmp::min(len, REPLY_BYTES);` (một dòng) — hoặc `min(len, room, REPLY_BYTES)`. |
| 2 | **TAP 4 (`shell ps tu core`) là xanh giả cho trường `state`.** Kernel (`proc_demo`) **tự tính lại** dòng kỳ vọng bằng **đúng công thức giải mã** của shell (`(entry>>1)&0x0F`) rồi so với output shell ⇒ đổi cách **đóng gói** thì cả hai sai giống nhau và test vẫn xanh. Đây đúng dạng D1 của KT-107, còn sót sau bản vá KT-105. | `v2/kernel/vifast-kbin-riscv64/src/proc_demo.rs:785-800` (giải mã) so với `:329` (đóng gói) và `init/shell_user.rs:266-270` | **nặng** | `kt115-logs/kt115-tap4-mutation.log` vs `kt115-tap4-baseline.log`: đột biến `state << 1` → `state << 2` ở `proc_demo.rs:329` ⇒ shell in `pid=15 state=Running` (đúng phải `Ready`) nhưng vẫn `ok 4 - shell ps tu core` + `snapshot_match=1`. Baseline in `state=Ready`. | Kỳ vọng phải lấy **trạng thái thật** từ `Machine` (`m.tcb_state(id)`) và so **nhãn** với output shell, **không** giải mã lại `entry`. |
| 3 | **`check_ps_no_unknown` (bản vá KT-112) là kiểm rỗng.** `n_rows` và `n_declared` cùng sinh từ **một** biến `count` của shell (`for entry in list.iter().take(count)` + `put_u64(count)`) ⇒ `n_rows == n_declared` luôn đúng; `ps` thoái hoá còn **một** tiến trình vẫn xanh. Tuyên bố trong comment ("Nay đòi **đúng** số dòng … `ps` thoái hoá còn một tiến trình vẫn xanh") **sai**. | `v2/test/boot/shell-edge.sh:196-206` | **vừa** | Chạy chính hàm trên log giả (`/tmp/kt115-se/t.sh`): `# shell: ps n=1` + 1 dòng `pid=13 state=Ready` ⇒ **PASS**; `n=3` + 1 dòng ⇒ FAIL. Log thật của ảnh sản phẩm là `n=3` + 3 dòng. | Đòi **tập pid** cụ thể (procsrv/shell/victim) hoặc `n_rows >= 3` viết tay; hoặc bỏ tuyên bố KT-112. |
| 4 | **Bất biến pager (`reads == misses`, D2-106) không thể làm đỏ cổng.** Chỉ in dòng `# pager: invariant …` **khi vi phạm**; `evidence_for ktest-user` **không** có mẫu này; `check_tap_common` chỉ đếm `ok`/`not ok`. Ngoài ra `sectors_per_page` tính bằng **hằng sản xuất** (`PAGE_SIZE / SECTOR_SIZE`) ⇒ dạng D1. | `v2/kernel/vifast-kbin-riscv64/src/ktest_vfs.rs:1398-1411`; `v2/test/boot/ktest.sh:636` (evidence) | **vừa** | `kt115-logs/kt115-pager-invariant-mutation.log`: đột biến `core.rs:158` `reads += 1` → `+= 2` ⇒ log có `# pager: invariant reads=14 misses=7 … ok=0` nhưng vẫn **35 `ok`, 0 `not ok`**, PANIC đúng mẫu ⇒ harness PASS. | Thêm TAP (hoặc mẫu `evidence_for`) đòi `reads == misses`; viết `sectors_per_page` bằng literal 8. |
| 5 | **`spec/v1-parity.toml` sai sự thật ở 4 mục sau KT-114.** Shell **đã có** `time`/`uptime`/`sync`/`stat` (HEAD `11550366`) nhưng spec vẫn `deferred-K7` với lý do "thieu BUILTIN time trong shell v2" / "shell v2 chua co lenh FS" / "v2 chua co fsync tu user mode". Spec là **nguồn sự thật** cho parity (AGENTS §1) ⇒ 4 mục đang nói sai. | `spec/v1-parity.toml:149` (`time`), `:227` (`stat`), `:239` (`sync`), `:298` (`uptime`) | **vừa** | `kt115-logs/kt115-new-shell-cmds.log`: boot ảnh `product-fs` build từ HEAD, gửi `time`/`uptime`/`sync`/`stat /README` ⇒ `# shell: time ns=612139000`, `# shell: uptime ms=613`, `# shell: sync ok`, `# shell: stat path=/README kind=1 size=79 ino=2`. Mã: `init/shell_user.rs:344-357`. | Chuyển 4 mục sang `active` (kèm `send`/`expect`) **hoặc** sửa `reason` thành "shell đã có builtin, chưa đo parity" — không để lý do cũ. |
| 6 | **`k6-parity.md` nói đường sản phẩm không có procsrv — sai từ KT-77b.** Ảnh sản phẩm canonical chạy procsrv và `ps` **do procsrv phục vụ**. | `v2/docs/k6-parity.md:196` | **vừa** | `kt115-logs/kt115-product-ps.log`: boot `v2/artifacts/0b79a652…bin` ⇒ `# proc: up proto=wait/kill`, `# shell: ps n=3` + 3 dòng `pid=… state=Ready`, `ps_replies=1 ps_n=3`. | Sửa §5: procsrv **có** trong ảnh sản phẩm từ KT-77b; `ps` không còn phụ thuộc ảnh test. |
| 7 | **`k6-parity.md` chép lại **công thức lỗi** của KT-105** (`state=(entry>>1)&0x1f`). Mã hiện tại dùng `& 0x0F` (`shell_user.rs:270`). Người đọc tài liệu sẽ tưởng lỗi còn. | `v2/docs/k6-parity.md:63` | **vừa** | `grep -n 'entry >> 1' init/shell_user.rs` ⇒ `:270 let state = (entry >> 1) & 0x0F;`; KT-105 đã vá cả shell + `proc_demo.rs:787`. | Sửa thành `& 0x0F` + ghi chú "KT-105 đã vá". |
| 8 | **Dẫn dòng cũ trong `k6-parity.md` và `proc_abi.rs`** (cùng dạng K6-DOD.md đã bị bắt đêm nay): `shell_user.rs:45` (thực tế là `};`), `:189-218` (là `send`/`recv`, không phải `ps`), `:247-269` (là `cmd_ps`, không phải 4 lệnh active), `:299` (là `put_u64(mr[1])`), `proc_demo.rs:270-280` (là `service()`, `build_script` ở `:244`), `proc_demo.rs:338` (là `];`). | `v2/docs/k6-parity.md:55,63,72,84,186,196,200`; `v2/kernel/vifast-kbin-riscv64/src/proc_abi.rs:8` | **nhẹ** | `sed -n '<n>p'` từng dòng ở HEAD `11550366` (xem §3.4). | Thay bằng **tên hàm** thay vì số dòng, hoặc cập nhật số dòng + thêm ca kiểm `docs-ref` (như `gen-check`). |
| 9 | **`entries[n]` không kiểm `n < 3`** trong `proc_rt` (bản `proc_demo` **có** `if n < entries.len()`). Hiện an toàn vì `ids` có đúng 3 phần tử, nhưng là bất biến ngầm: thêm một TCB thứ tư là panic. | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:256` | **nhẹ** | Đọc mã: `ids = [T_PROCSRV, T_SHELL, T_VICTIM]` (3) + `entries = [0u64; 3]`; `n` chỉ tăng khi `tcb_state` Ok ⇒ `n ≤ 2` lúc ghi. So sánh `proc_demo.rs:330` có guard. | Thêm `if n < entries.len()` như `proc_demo` (một dòng). |
| 10 | **Shell tin `len`/`count` của kernel mà không kẹp**: `put(&name[..len])` / `put(&buf[..count])` với `len`/`count` đọc từ reply. Kernel hiện kẹp ≤ 24 nên an toàn, nhưng nếu kernel đổi (hoặc một client khác gửi reply) thì **panic U-mode** (exit 99 ⇒ kernel đỏ). | `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs:472,482,518,521` | **nhẹ** | Đọc mã: `unpack_bytes` kẹp nội bộ nhưng `name[..len]`/`buf[..count]` dùng số **chưa** kẹp. | `let len = len.min(FS_BYTES);` trước khi in (hai dòng). |
| 11 | **Đường dẫn > 48 B bị cắt im lặng.** `load_path` kẹp `len` theo `room` nhưng **không** trả lỗi; shell bỏ qua `len` trả về trong reply ⇒ tên bị cắt mà shell tưởng đủ. | `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs:449-459`; `init/shell_user.rs:424-441` | **nhẹ** | Đọc mã. Hiện khó trúng vì `MAX_NAME = 28` (`vifast-fs/src/consts.rs:33`) nên đường dẫn hợp lệ ở `/` ≤ 29 B; nhưng khi có `mkdir` (K7) là trúng. | Trả `ST_ERR` (`NameTooLong`) khi `len` bị kẹp, hoặc shell kiểm `reply[1] == count`. |
| 12 | **Comment trong mã nói "Không bao giờ panic" — sai** (bị #1 bác). | `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs:419` | **nhẹ** | Cùng log #1. | Sửa comment sau khi vá #1. |
| 13 | **`machine.rs` 1497/1500 dòng, `user.rs` 1489/1500** — sát trần R1 (`v2/bin/v2:145-156`). Thêm **4** dòng vào `machine.rs` (hoặc **12** dòng vào `user.rs`) là `v2 check` ĐỎ; task sau buộc phải tách tệp giữa việc khác. | `v2/kernel/vifast-core/src/machine.rs`; `v2/kernel/vifast-plat-riscv64/src/user.rs` | **vừa** | `wc -l` = 1497 / 1489; R1 = `> 1500` ⇒ đỏ. | Tách `machine.rs` (phần `check_*`/`footprint`) và `user.rs` (phần `exec`/`load`) **trước** khi cần, không đợi chạm trần. |
| 14 | **`kobj.rs` 11.875 dòng** — tệp lớn nhất dự án, chứa **toàn bộ** mô hình object của sim (249 hàm), **không** thuộc R1 (R1 chỉ quét `kernel/` + `lib/`). Đây là oracle của kernel ⇒ mọi thay đổi ngữ nghĩa phải sửa **hai** nơi (`vifast-core` 13.570 dòng + `kobj.rs` 11.875 dòng) và giữ khớp bằng 15.128 dòng test parity. | `v2/sim/vifast-sim/src/kobj.rs` | **vừa** | `wc -l`; `grep -c '^pub fn \|^    pub fn \|^fn '` = 249. | Mở R1 cho `sim/` (hoặc luật riêng ≤ 3.000 dòng) rồi tách `kobj.rs` theo miền (cap/untyped/ipc/sched). |
| 15 | **Trùng lặp `build_rig` + `service` giữa ảnh sản phẩm và ảnh test.** Hai bản cùng dựng cspace 6–7 endpoint + 4–5 TCB + SC + SchedControl và cùng xử lý `PS_REQ`/`EV_*`; sửa một bên không sửa bên kia là lệch bố cục slot (đúng loại lỗi `proc_abi` sinh ra để chặn). | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:142-209,213-293` vs `src/proc_demo.rs:152-215,263-344` | **vừa** | `diff` hai đoạn `build_rig`: 63/68 dòng khác nhau chủ yếu ở chuỗi `expect` + số TCB; logic giống. | Đưa `build_rig`/`service` vào **một** module dùng chung (tham số hoá `N_TCB`, có/không `EP_FS`). |
| 16 | **Hằng `proc_abi` chép tay trong 3 chương trình U-mode** (`shell_user.rs:49-52`, `procsrv_user.rs:29-32`, `child_user.rs`). Đã biết và có script quét (`kt77b-harness-scan.sh`), nhưng script **không** nằm trong `gate --fast` ⇒ lệch slot chỉ bị bắt khi có người chạy tay. | `v2/kernel/vifast-kbin-riscv64/init/*.rs`; `v2/agent/JOURNAL/kt77b-harness-scan.sh` | **nhẹ** | Đọc mã + `grep -n 'const EP_' init/*.rs`. | Thêm bước quét slot vào `v2 check` (rẻ, chỉ grep ELF). |
| 17 | **`entries[1] << 16` mất bit nếu `ObjId ≥ 2^11`.** Gói 3 entry vào 2 word 16 bit; `id << 5` tràn qua bit 16 khi `id ≥ 2048`. Hiện `ObjId` nhỏ (13–21) nên không trúng. | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:260`; `src/proc_demo.rs:335-338` | **nhẹ** | Đọc mã. | Ghi bất biến `debug_assert!(id < (1<<11))` hoặc gửi 3 word. |
| 18 | **`fs_call` có thể ăn reply cũ sau một lần quá hạn.** Hết `FS_REPLY_BUDGET` ⇒ trả `None` nhưng request vẫn nằm trong hàng đợi kernel; reply đến muộn sẽ được lệnh **kế** nhận (cùng label `FS_REPLY`) ⇒ kết quả sai lệch. | `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs:388-402` | **nhẹ** | Đọc mã. Chỉ xảy ra khi FS chậm/quá hạn (đã là trạng thái lỗi). | Sau khi quá hạn, đặt cờ "mất đồng bộ" và bỏ reply đầu tiên của lệnh kế; hoặc thêm `seq` vào reply. |
| 19 | **`svc.fs_replies` tăng cả khi `nb_send` thất bại** ⇒ dòng evidence `fs_replies=` có thể đếm cả reply **không** gửi được. | `v2/kernel/vifast-kbin-riscv64/src/proc_rt.rs:284-285` | **nhẹ** | Đọc mã: `let _ = m.nb_send(…); svc.fs_replies += 1;`. | `if m.nb_send(…).is_ok() { svc.fs_replies += 1; }`. |

**Tổng theo mức**: **chặn 0 · nặng 2 · vừa 8 · nhẹ 9** (19 phát hiện).

> Ghi chú phân loại: #1 là **nặng** chứ chưa **chặn** vì shell phát hành **không** gửi `len > 24`
> (`fs_set_path` luôn `count ≤ FS_BYTES`). Nhưng `EP_FS` nằm trong cspace **dùng chung** của
> procsrv + shell (`proc_rt.rs:142-209` cấu hình **một** cspace cho mọi TCB) ⇒ procsrv **cũng có**
> cap đó; một lỗi ở procsrv là đủ giết kernel. Nếu sau này có client thứ hai gửi `OP_PATH`, #1
> thành **chặn**.

---

## 2. Chi tiết ba phát hiện nặng nhất

### 2.1 #1 — `proc_fs.rs:454`: PANIC kernel từ một request U-mode

**Mã** (`v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs:444-460`):

```rust
fn load_path(&mut self, words: &[u64; 4], len: usize) -> [u64; 4] {
    ...
    let room = MAX_PATH.saturating_sub(self.path_len);   // ≤ 48
    let len = core::cmp::min(len, room);                 // ← kẹp theo 48, KHÔNG theo 24
    ...
    while index < len {
        let word = words[1 + index / 8];                 // ← index = 24 ⇒ words[4]
```

`len` đến từ `head >> 40` (`proc_fs.rs:424`, 8 bit ⇒ ≤ 255). Ba MR word chỉ mang **24 B**
(`REPLY_BYTES`). Với `len ≥ 25`, vòng lặp chạm `words[4]` trên `[u64; 4]` ⇒ panic biên.

**Ca chạy được** (bản clone `/tmp/kt115-repo`, HEAD `11550366`): sửa **một dòng** trong
`init/shell_user.rs` (`fs_set_path`) thành `let head = OP_PATH | (25u64 << 40);`, build
`--features product-shell,product-fs`, boot QEMU, gửi `ls`:

```text
# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/time/uptime/sync/stat/exit
# shell: input=ring va=0x43000000
shell>
PANIC at vifast-kbin-riscv64/src/proc_fs.rs:454:24
PANIC: index out of bounds: the len is 4 but the index is 4
```

Log đầy đủ: `v2/agent/JOURNAL/kt115-logs/kt115-oob-panic.log`. Máy **chết** (không halt sạch,
không có dòng `# product halt:`), tức là một request sai của U-mode biến thành **DoS kernel**.

**Vá một dòng**: `let len = core::cmp::min(len, REPLY_BYTES);` (thay cho `min(len, room)`), hoặc
`min(len, room, REPLY_BYTES)`. Kèm ca kiểm: gửi `OP_PATH` với `len = 25` phải trả `ST_ERR`/`ST_OK`
với `len = 24`, **không** panic.

### 2.2 #2 — TAP 4 xanh giả: test tự giải mã lại đúng công thức của shell

**Mã**: `proc_demo.rs:329` đóng gói `(id << 5) | (state << 1) | suspended`; shell giải mã
`(entry >> 1) & 0x0F` (`shell_user.rs:270`); TAP 4 (`proc_demo.rs:785-800`) **giải mã lại y hệt**
rồi so với chuỗi shell đã in. Ba chỗ dùng **một** công thức ⇒ đổi công thức ở chỗ đóng gói thì
shell in sai **và** kỳ vọng sai giống nhau.

**Ca chạy được**: đột biến `proc_demo.rs:329` `<< 1` → `<< 2` (lỗi thật: mọi trạng thái bị nhân
đôi), build `--features ktest,user --bin vifast-kbin-ktest-proc`, boot:

```text
# shell: ps n=4
  pid=15 state=Running      ← đúng phải là Ready
  pid=17 state=Running
  pid=19 state=Inactive
  pid=21 state=Running
# shell: ps replies=1 entries=4 snapshot_match=1 captured=1
ok 4 - shell ps tu core
```

Baseline (không đột biến) cùng ảnh: `pid=15 state=Ready`, `pid=17 state=Ready`,
`pid=19 state=Inactive`, `pid=21 state=Ready`, vẫn `ok 4`. Log:
`kt115-logs/kt115-tap4-mutation.log` + `kt115-logs/kt115-tap4-baseline.log`.

**Vì sao quan trọng**: đây **đúng** dạng KT-105 (đêm nay) — bản vá KT-105 sửa công thức ở **cả
hai** phía nhưng **không** thêm nguồn độc lập, nên lỗi đóng gói vẫn lọt. KT-107 đã audit 30 ứng
viên nhưng **bỏ sót** chính chỗ này (nó chỉ nhắc `proc_demo.rs:787` như "đã vá", không kiểm lại
tính độc lập sau vá).

**Vá**: kỳ vọng lấy từ `m.tcb_state(id)` (nguồn thật) rồi so **nhãn** (`state_label`) với output
shell; **không** giải mã `entry` ở phía test.

### 2.3 #3 — `check_ps_no_unknown`: bản vá KT-112 không bắt được điều nó tuyên bố

**Mã** (`v2/test/boot/shell-edge.sh:190-212`): `n_rows` đếm dòng `  pid=… state=…`, `n_declared`
đọc `# shell: ps n=<n>`. Nhưng shell in **cả hai** từ **một** biến `count`
(`shell_user.rs:262-296`: `for entry in list.iter().take(count)` rồi `put_u64(count)`) ⇒
`n_rows == n_declared` **luôn đúng**; nếu `ps` chỉ liệt kê 1 tiến trình thì `n=1` và `n_rows=1`
⇒ PASS.

**Ca chạy được** (chạy chính hàm của harness trên log giả, `/tmp/kt115-se/t.sh`):

```text
a.log -> PASS     # '# shell: ps n=1' + '  pid=13 state=Ready'
b.log -> PASS     # '# shell: ps n=3' + 3 dòng
c.log -> FAIL (ps khai n=3 nhưng in 1 dòng pid — bảng thiếu dòng)
```

Tức là ca "ps thoái hoá còn một tiến trình" (đúng ca KT-112 tuyên bố đã bắt) **vẫn xanh**. Comment
tại `shell-edge.sh:196-198` nói "hai nguồn độc lập trong cùng log" — **không** độc lập.

**Vá**: đòi **tập pid** cụ thể (13/15/17 trên ảnh đã ghim) hoặc `n_rows >= 3` viết tay; nếu giữ
`n_declared` thì phải lấy từ nguồn khác (ví dụ đếm `pid=` trong `# product shell:` của kernel).

---

## 3. Chi tiết các phát hiện vừa

### 3.1 #4 — bất biến pager không thể làm đỏ cổng

`ktest_vfs.rs:1398-1411` chỉ **in** dòng `# pager: invariant …` khi vi phạm; `evidence_for
ktest-user` (`ktest.sh:636`) **không** có mẫu này; `check_tap_common` chỉ đếm `ok`/`not ok`/PANIC.
Đột biến `v2/lib/vifast-vfs/src/core.rs:158` `self.reads += 1` → `+= 2`, build `ktest-user`, boot:

```text
# pager: invariant reads=14 misses=7 sectors=56 const:sectors_per_page=8 ok=0
...
PANIC: KT-70 u-mode pha syscall, invocation, capalloc, IDL va ring va pager xong (35 TAP, 0 fail)
```

`grep -c '^ok '` = **35**, `grep -c '^not ok'` = **0** ⇒ harness PASS. Log:
`kt115-logs/kt115-pager-invariant-mutation.log`. Ngoài ra `sectors_per_page` tính bằng
`PAGE_SIZE / vifast_vfs::SECTOR_SIZE` (hằng sản xuất) ⇒ nếu `SECTOR_SIZE` sai thì cả hai phía sai
giống nhau (dạng D1); may là `# pager: cache … sectors=56` trong `evidence_for` có literal 56 nên
phần `sectors` vẫn được kiểm **độc lập** — chỉ `reads == misses` là không.

### 3.2 #5 — `spec/v1-parity.toml` lệch mã sau KT-114

HEAD `11550366` thêm `time`/`uptime`/`sync`/`stat` (`init/shell_user.rs:344-357`, `help` in
`cmds=13`). Boot ảnh `product-fs` build từ HEAD:

```text
# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/time/uptime/sync/stat/exit
# shell: time ns=612139000
# shell: uptime ms=613
# shell: sync ok
# shell: stat path=/README kind=1 size=79 ino=2
```

Nhưng `spec/v1-parity.toml` vẫn:

- `:149` `time` — `deferred-K7`, reason "thieu BUILTIN time trong shell v2 (`shell_user.rs:247-269` …)"
  (dòng 247-269 giờ là `cmd_ps`, không phải danh sách lệnh).
- `:298` `uptime` — `deferred-K7`, reason "thieu BUILTIN uptime trong shell v2".
- `:239` `sync` — `deferred-K7`, reason "shell v2 chua co lenh FS/flush; **v2 chua co fsync tu user
  mode**" — sai: `write` đã gọi `OP_SYNC` từ KT-77c, và `sync` mới gọi trực tiếp.
- `:227` `stat` — `deferred-K7`, reason "shell v2 chua co lenh FS" — sai: `OP_STAT` có từ KT-77c và
  `stat` mới gọi nó.

Đêm nay đã bắt **năm** `reason` cũ; đây là **bốn** mục nữa, và ba trong bốn **mới sai thêm** vì
KT-114 vừa land mà không cập nhật spec.

### 3.3 #6/#7 — `k6-parity.md` nói sai về đường sản phẩm

- `:196` "đường **sản phẩm** **không** có procsrv" — sai từ KT-77b. Boot ảnh canonical
  `0b79a652…`: `# proc: up proto=wait/kill`, `# shell: ps n=3`, `ps_replies=1 ps_n=3`
  (`kt115-logs/kt115-product-ps.log`).
- `:63` chép công thức **lỗi** `state=(entry>>1)&0x1f`; mã hiện tại `& 0x0F` (`shell_user.rs:270`).
- `:55` "**Bốn** lệnh `active`" — spec hiện có **6** (`ls`, `cat` thêm từ KT-77c).

### 3.4 #8 — dẫn dòng cũ (cùng dạng K6-DOD.md)

Đọc từng dòng ở HEAD `11550366`:

| Tài liệu | Dẫn | Thực tế tại dòng đó |
|---|---|---|
| `k6-parity.md:55` | `shell_user.rs:247-269` = 4 lệnh active | `cmd_ps` (4 lệnh ở `run_line`, `:671+`) |
| `k6-parity.md:63` | `shell_user.rs:189-218` = `ps` decode | `send`/`recv` (decode ở `:260-275`) |
| `k6-parity.md:72` | `shell_user.rs:299` = xử lý `\n` | `put_u64(mr[1])` (xử lý `\n` ở `:700+`) |
| `k6-parity.md:84,186` | `proc_demo.rs:270-280` = `build_script` | `service()` (`build_script` ở `:244`) |
| `k6-parity.md:196` | `proc_demo.rs:338` = chỗ phục vụ `PS_REQ` | `];` (handler ở `:311-344`) |
| `k6-parity.md:200` | `shell_user.rs:45` = `CMDS` | `};` (`CMDS` ở `:102`) |
| `proc_abi.rs:8` | `procsrv_user.rs:29`, `shell_user.rs:45` = hằng endpoint | `procsrv_user.rs:29` = `const EP_FAULT` (đúng); `shell_user.rs:45` = `};` (hằng ở `:49-52`) |

---

## 4. Năm việc nên làm trước

1. **Vá #1 (`proc_fs.rs:450`)** — một dòng, chặn một đường panic kernel từ U-mode. Vì sao trước:
   đây là lỗi **đúng/sai** duy nhất trong báo cáo có thể giết máy, và bản vá rẻ nhất (thay `room`
   bằng `REPLY_BYTES`). Kèm ca kiểm `len = 25` (hiện chưa có ca nào chạm biên này).
2. **Vá #2 (TAP 4)** — vì nó là **cùng dạng lỗi KT-105** mà đêm nay vừa coi là đã xử lý; nếu để
   lại, mọi bản vá "sửa công thức ở hai phía" tiếp theo sẽ tiếp tục cho cảm giác an toàn giả. Sửa
   bằng cách so với `m.tcb_state(id)` (nguồn thật), không giải mã `entry`.
3. **Cập nhật `spec/v1-parity.toml` cho 4 lệnh mới (#5)** — spec là **nguồn sự thật** cho parity
   (AGENTS §1); để `deferred-K7` kèm lý do "chưa có" khi shell **đã có** là tự tạo tài liệu sai,
   đúng loại lỗi K6-DOD.md mà đêm nay đã bắt. Nên chuyển `time`/`uptime`/`sync`/`stat` sang
   `active` **kèm** `send`/`expect` đo được (đã có output thật ở §3.2).
4. **Sửa #3 (`check_ps_no_unknown`)** — hoặc làm nó thật sự bắt được "ps còn một tiến trình", hoặc
   **bỏ** tuyên bố KT-112 trong comment. Một harness tự nhận bắt lỗi mà không bắt được là nợ
   niềm tin lớn hơn một ca kiểm thiếu.
5. **Ép #4 thành TAP/evidence** — bất biến `reads == misses` (D2-106) hiện **không thể** làm đỏ
   cổng; thêm một dòng vào `evidence_for ktest-user` (hoặc một TAP) là đủ, và viết
   `sectors_per_page` bằng literal 8 để không dùng chung công thức với code.

**Việc thứ sáu (rẻ, nên gộp vào lần sửa tài liệu kế)**: sửa `k6-parity.md` (#6/#7/#8) — 3 câu sai
sự thật + 7 dẫn dòng cũ; cùng lúc thêm một ca kiểm `docs-ref` (grep `file:dòng` trong `v2/docs`
rồi so với mã) để không tái diễn.

---

## 5. Đã kiểm nhưng **KHÔNG** thấy lỗi (để người sau khỏi kiểm lại)

### 5.1 Đường lỗi trên dữ liệu ngoài

- **`vifast-elf` parser** (`v2/lib/vifast-elf/src/lib.rs:175-323`): `phoff/phentsize/phnum` đều
  `checked_mul`/`checked_add`, `phnum ≤ MAX_LOAD_SEGMENTS`, `file_end`/`memory_end` kiểm biên,
  `image.segments[image.count]` không thể tràn (mỗi vòng +1, `phnum ≤ 16`). Không `unwrap`/`expect`
  ngoài test. **Không thấy lỗi.**
- **`vifast-fs`** (`v2/lib/vifast-fs/src/*.rs`): chỉ **một** `panic!` trong `crash.rs:111` (test
  helper). `blocks_used()` kẹp `MAX_BLOCKS_PER_INODE` cho thư mục (đã vá vòng 83). `read`/`write`
  dùng `checked_add`; `read_dir_entry_full` kiểm `index < total_slots`, `len ≤ MAX_NAME`,
  `off+3+len ≤ BLOCK_SIZE`. `format_with_options` kiểm `block_count() < DATA_START+2`. **Không
  thấy lỗi.**
- **`UserRange::contains`** (`vifast-plat-common/src/user.rs:354-368`) và
  **`UserRanges::contains`** (`vifast-plat-riscv64/src/user_ranges.rs:59-75`): `checked_add`,
  `len == 0` xử lý riêng. **Không thấy lỗi.**
- **`mmu::walk_with`** (`vifast-plat-common/src/mmu.rs:258-302`): `is_canonical` trước, `load`
  trả `Option`, `level` giảm có điều kiện. **Không thấy lỗi.**
- **`ipc_buffer::read_header`** (`vifast-plat-riscv64/src/ipc_buffer.rs`): chỉ số cố định
  `IPC_HEADER_W_*`, kiểm `user_range_mapped` trước khi đọc. **Không thấy lỗi.**
- **`build_message`** (`vifast-core/src/syscall.rs:270-304`): `len > mrs.len()` ⇒ lỗi,
  `n_caps ≤ MAX_CAPS`, `header.send_caps.iter().take(n_caps)`. **Không thấy lỗi.**
- **`core_sched`**: mọi `self.sched[hart]`/`self.published[hart]` đều có `hart >= n_harts` chặn
  trước (`:31,45,190,247`). **Không thấy lỗi.**
- **`machine` accessor**: `obj == 0 || obj > n_objects` chặn trước khi index
  (`machine.rs:624-646`). **Không thấy lỗi.**
- **`conchan::pack/unpack`** (`vifast-virtio/src/conchan.rs`): `count` kẹp `CHUNK_BYTES`, `unpack`
  từ chối `count == 0 || count > 24`. **Không thấy lỗi.**
- **`uart_console::drain_rbr`** (`:333-355`): `count` reset khi `== CHUNK_BYTES` ⇒ không tràn
  `raw[24]`; `push` xử lý ring đầy bằng `drops` + `FULL_CODE` (không ghi đè). **Không thấy lỗi.**
- **`procsrv_user::put_u64`/`record_exit`**: `buf[20]` đủ cho `u64::MAX` (20 chữ số);
  `EXIT_PID[8]` có `if n < 8`. **Không thấy lỗi.**
- **`shell_user::parse_u64`**: `checked_mul`/`checked_add`. **Không thấy lỗi.**
- **`shell_user` dòng lệnh**: `len < MAX_LINE` trước khi ghi, cờ `overflowed` báo một lần
  (KT-103). **Không thấy lỗi.**
- **`proc_demo::set_line`/`decimal`**: `n = min(text.len(), 24)`, `decimal` ghi vào `wait[5..]`
  (19 B) với pid `u32` (≤ 10 chữ số). **Không thấy lỗi.**
- **`StaticSlot::init_with`**: `compare_exchange` một lần ghi, trả `None` lần hai. **Không thấy
  lỗi.**
- **`load_init_slot`**: mọi đường lỗi sau khi cấp VSpace/Frame đều đi qua `bail!` →
  `release_with` (không mồ côi). **Không thấy lỗi.**

### 5.2 Test / harness

- **`ktest.sh` khung TAP** (`check_tap_common`): đòi đúng 1 `TAP version 13`, đúng 1 `1..N`, dãy
  `ok 1..N` **liên tục theo thứ tự** (bắt thiếu/trùng/đảo), `not ok == 0`, đúng 1 PANIC đúng mẫu
  của ảnh. **Không thấy lỗi.**
- **`ktest.sh` `check_trace`**: tập `step` so với `check_scn.py --steps` (nguồn độc lập), bắt
  trùng/thiếu/định dạng. **Không thấy lỗi.**
- **`ktest.sh` `check_stack_lines`**: parse hex bằng bash `16#`, fail-closed khi không parse đủ
  dòng (đã vá lỗi `strtonum` của BSD awk). **Không thấy lỗi.**
- **`budget.sh`**: `--selftest` 3 fixture (cộng đúng, tên biến thể `.data.rel.ro`/`.sbss.*`, ca
  vượt trần). **Không thấy lỗi.**
- **`shell-edge.sh` `--selftest`**: 8 ca, mỗi ca một log **sai** (đúng đầu ra lỗi cũ) + một log
  **đúng**, cộng 3 ca phụ "vừa đúng vừa sai" và ca 8 "đúng nhưng chậm 99 s". **Không thấy lỗi**
  (trừ #3 ở trên).
- **`persist_host.py`**: KT-111 đã thêm 4 bất biến **ngữ nghĩa** (single != double, cả hai > 0,
  detail > cả hai, detail ≥ 2^32) **trước** khi kiểm công thức; commit KT-111 ghi đã đo `<<16` và
  `<<24` đều ĐỎ. Tôi **không** chạy lại phép đột biến đó (ngoài ngân sách) — đọc mã thấy 4 bất biến
  độc lập với công thức đóng gói. **Không thấy lỗi.**
- **`parity/run.sh`**: 3 ca âm (`wrong-expect`, `no-send`, `image product`), `serial_min_count`,
  `qemu_con_sot_total`. **Không thấy lỗi.**
- **`qemu_guard.sh`**: process group riêng, timeout kill cả group, watchdog khi cha chết, registry
  PGID; `guard_selftest.sh` có ca `QG_FORCE_NO_SETPGRP` để chứng minh bộ đếm **có thể** khác 0.
  **Không thấy lỗi.**
- **`ktest_vfs.rs` TAP 30-35**: kỳ vọng là literal (`a_ok_pages == 5`, `reject_page == 99`,
  `reject_status == InvalidArgument`, `neg_aux == 2`, `neg_over == AlreadyMapped`) — độc lập với
  công thức sản xuất. **Không thấy lỗi** (trừ #4).
- **`ktest_user.rs` TAP 7-13**: `mk_info(len) = len << 28` là **literal từ spec** (`abi.toml`
  `length:28..34`), không phải hằng sản xuất ⇒ đổi `pack`/`unpack` của core sẽ làm test đỏ (đúng
  hướng). Kỳ vọng `e_empty == 1`, `e_tcb == 2`, `e_call == 18`, `e_recv == 2` là literal.
  **Không thấy lỗi.**
- **`ktest_tlb.rs`**: `sent_expected` của kernel được harness kiểm bằng `__TLB_SENT__ =
  100*(smp-1)` (nguồn độc lập: `smp` do người chạy chọn); `acked_of_mask` chỉ so trước/sau.
  **Không thấy lỗi.**
- **`ktest_persist.rs:770` `raw[10]`/`raw[11]`**: `inode_blocks_raw` trả `[u32; 12]` ⇒ trong biên.
  **Không thấy lỗi.**

### 5.3 Giao thức FS của ảnh sản phẩm

- `OP_LS`/`OP_CAT`/`OP_WRITE`/`OP_TRUNC`/`OP_STAT`/`OP_SYNC`: mọi chỉ số `words[1 + i/8]` /
  `data[i/8]` có `i < 24` (sau khi kẹp `REPLY_BYTES`) ⇒ trong biên. `offset`/`size` là 32 bit nên
  không tràn `usize` trên riscv64. **Không thấy lỗi** (trừ #1 ở `OP_PATH`).
- `err_code` phủ **đủ 14** biến thể `FsError` (không có nhánh `_`). **Không thấy lỗi.**
- `Medium::volatile`/`name`, `prepare()` 3 nhánh (Disk/Ram/Nothing) đều có đường ra, không treo.
  **Không thấy lỗi.**
- `RamVol::new` kiểm **8 B cuối** volume nằm trong physmap. **Không thấy lỗi.**

### 5.4 Tài liệu khác đã đối chiếu (không thấy sai)

- `v2/docs/k6-product-fs.md` §4/§6/§7: các dòng `# product fs:` / `# shell: cat ok` khớp mã
  (`proc_fs.rs:391-405`, `:512-540`) và khớp log boot lại của tôi.
- `v2/docs/k6-build-rev.md` + `build-rev.sh`: dấu vân tay theo **nội dung** (không theo lịch sử),
  `dirty` fail-closed; build lại từ `/tmp/kt115-repo` cho `inputs=f5f7de891e796189` khớp HEAD.
- `v2/docs/k6-repro.md`: build hai lần ở hai `CARGO_TARGET_DIR` cho cùng sha. Tôi build lại
  `product-fs` từ HEAD ⇒ `152f59aa…`, khớp `inputs=f5f7de891e796189` (không tự đo lại tính tái lập
  hai lần — ngoài ngân sách).
- `v2/docs/k35-procsrv.md` §2 ("kernel đóng vai scheduler tuần tự") khớp mã: `proc_rt::turn` +
  `MULTITASK` shortcut cho `Yield` (`user.rs:868-880`).

---

## 6. Phụ lục — cách tái lập

```bash
# 1) clone + build (không đụng repo)
git clone --shared /Users/mps/dvchd/ViFastOs /tmp/kt115-repo
cd /tmp/kt115-repo && export PATH="$HOME/.cargo/bin:$PATH"
VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
  bash v2/tools/repro-build.sh --root /tmp/kt115-repo --target-dir /tmp/kt115-target

# 2) #1: sửa MỘT dòng trong bản clone rồi build lại
#    init/shell_user.rs:  let head = OP_PATH | ((count as u64) << 40);
#                     ->  let head = OP_PATH | (25u64 << 40);
#    boot + gửi "ls"  =>  PANIC at .../proc_fs.rs:454:24

# 3) #2: sửa proc_demo.rs:329  << 1  ->  << 2, build --features ktest,user \
#         --bin vifast-kbin-ktest-proc, boot  =>  ok 4 vẫn xanh, state in sai

# 4) #4: sửa v2/lib/vifast-vfs/src/core.rs:158  reads += 1 -> += 2,
#         build --features ktest,user --bin vifast-kbin-ktest-user, boot
#         =>  "# pager: invariant ... ok=0"  nhưng 35 ok / 0 not ok

# 5) #3: chạy chính hàm của harness trên log giả
sed -n '/^check_ps_no_unknown()/,/^}/p' v2/test/boot/shell-edge.sh > /tmp/fn.sh
#    nguồn: /tmp/kt115-se/t.sh (a.log PASS, b.log PASS, c.log FAIL)
```

QEMU dùng: `-machine virt,aia=none,aclint=on -bios <opensbi> -kernel <img> -nographic -cpu rv64
-m 128M -smp 1 -no-reboot`, stdin là FIFO bơm payload sau 1 s; mỗi lần chỉ **một** QEMU, `kill`
đúng PID của script. Log bằng chứng: `v2/agent/JOURNAL/kt115-logs/`.
