# KT-137 — `cd`/`pwd` + thư mục hiện tại + đường dẫn tương đối

**Ngày**: 2026-11-18/19 · **Task**: KT-137 (hàng QUEUE, D2-133) · **Nhánh**: `night/L5-kt137`
(tách từ `main` tại `a9460f0c`) · **Worktree**: `/tmp/night-L5-kt137`.

**Quyết định**: D2-133 (Agent 1 quyết thay Agent 0) — KT-132 đã chứng minh v1 tại tag đóng băng
**có** `cd`/`pwd` chạy thật (`cd /bin` rồi `pwd` ⇒ `/bin`, log `JOURNAL/kt132-v1-cdpwd.log`), nên
hai lệnh thuộc "tương đương v1" và phải có trong shell v2. Phạm vi giữ nguyên như hàng QUEUE.

**Commit**: `e179450b` — `[KT-137] cd/pwd + thu muc hien tai + duong dan tuong doi trong shell san
pham (CMDS 15->17) …` (code + ca kiểm + `spec/v1-parity.toml` chỉ đổi `status` + hai tài liệu).

---

## 1. Kết luận ngắn

- Shell sản phẩm có **thêm hai builtin** `cd`/`pwd` (chỉ nhánh `cfg(shell_fs)`): `CMDS` **15 → 17**,
  `help cmds=17`, banner `# shell: up cmds=…/cd/pwd/…`. Ảnh test (không `shell_fs`) **vẫn 6** —
  đã kiểm bằng chuỗi trong ảnh `ktest-proc` build lại (§7).
- Shell giữ **thư mục hiện tại** trong buffer cố định (`Cwd`, 48 B = `proc_fs::MAX_PATH`) trên
  stack của `_start` — **không** `static mut`, **không** `unsafe`, **không** `alloc`.
- Mọi lệnh có tham số đường dẫn (`ls`/`cat`/`write`/`truncate`/`stat`) nhận **đường tương đối**;
  shell **tự chuẩn hoá từ vựng** (`.`/`..`/`//`) rồi mới gửi `OP_PATH`. **Không** sửa một dòng nào
  trong `v2/lib/vifast-fs/**` hay `v2/kernel/**/proc_fs.rs` ⇒ **không** đổi ngữ nghĩa FS, **không**
  đổi giao thức FS server, **không** mở ABI.
- Ba vế đủ: đỏ trên ảnh **mã cũ** (§4), xanh trên ảnh **mã mới** (§5), **đột biến** bỏ kiểm
  `kind=2` ⇒ ca kiểm đỏ (§6).
- `./v2/bin/v2 check` **PASS** (§8). `bash v2/test/boot/shell-edge.sh --selftest` **PASS 42/42**
  (§7). Toàn bộ **15 ca cũ** của `shell-edge` vẫn xanh (§5).

## 2. Vì sao **chỉ** sửa shell (ràng buộc kỹ thuật, không phải lựa chọn thẩm mỹ)

`v2/lib/vifast-fs/src/path.rs` **từ chối** hai tên `.`/`..` (`path.rs:148`) và đòi đường **tuyệt
đối** (`path.rs:123`). Vì vậy nếu shell gửi nguyên `README` hay `../README` xuống FS thì FS trả
`Invalid` (mã 2) — đúng hành vi **trước** KT-137. Cách duy nhất **không** đổi FS là shell tự phân
giải **từ vựng** (không chạm đĩa) rồi gửi đường tuyệt đối đã chuẩn hoá. Đó là cách đã làm.

Hệ quả đã biết (ghi để không ai đọc nhầm): `cd /khongco/..` **thành công** (về `/`) vì chuẩn hoá
là **từ vựng**, không kiểm sự tồn tại của từng thành phần — giống `cd` của POSIX ở mức lexical.
Ca kiểm `kt137-cdpwd` ghim đúng hành vi này.

## 3. Hành vi đã ghim (mỗi dòng là một phép kiểm của ca `kt137-cdpwd`)

| lệnh | đầu ra | ghi chú |
|---|---|---|
| `pwd` | `# shell: pwd /` | in thư mục hiện tại (đã chuẩn hoá) |
| `cd /khongco` | `# shell: cd khong co /khongco` | **không** đổi thư mục hiện tại |
| `cd /README` (tệp) | `# shell: cd khong phai thu muc /README` | **không** đổi thư mục hiện tại |
| `cd ..` ở gốc | `# shell: cd /` | ở `/` thì **ở lại** `/` |
| `cd` (không tham số) | `# shell: cd /` | chọn "về `/`" và **ghim** bằng ca kiểm |
| `cd /` | `# shell: cd /` | |
| `cd /README/..` | `# shell: cd /` | `..` chuẩn hoá từ vựng |
| `cd /khongco/..` | `# shell: cd /` | như trên (xem §2) |
| `cat README` | `# shell: cat ok bytes=79 first=ViFastOs` | đường **tương đối** |
| `cat ../README` | `# shell: cat ok bytes=79 first=ViFastOs` | `..` trong tham số |
| `stat README` | `# shell: stat path=/README kind=1 size=79 ino=2` | in đường **đã phân giải** |
| `write rel.txt xin chao` | `# shell: write /rel.txt bytes=8 sync=ok` | tạo ở thư mục hiện tại |
| `cat rel.txt` | `# shell: cat ok bytes=8 first=xin chao` | |
| `truncate rel.txt 2` | `# shell: truncate path=/rel.txt size=2` | |
| `stat rel.txt` | `# shell: stat path=/rel.txt kind=1 size=2 ino=5` | |
| `ls` (không tham số) | `# shell: ls n=4 path=/` | liệt kê **thư mục hiện tại** |
| `ls .` | `# shell: ls n=4 path=/` | `.` = thư mục hiện tại |

Thông điệp lỗi mới (đều bắt đầu `# shell: ` như mọi dòng khác): `thieu duong dan` (tham số rỗng),
`duong qua dai` (đường sau khi ghép > 48 B), `cd khong co <p>` (mã 5), `cd khong phai thu muc <p>`
(kind ≠ 2), `cd loi code=<n> <p>` (mã FS khác, ví dụ 7 = NotDir).

## 4. Vế 1 — **đỏ trên mã cũ** (ảnh build từ `a9460f0c`, trước KT-137)

Ảnh: worktree tạm `/tmp/kt137-old` (detached `a9460f0c`), build bằng chính `repro-build.sh`:

```text
REPRO machine=Darwin-arm64 root=/tmp/kt137-old sha256=93b8c3b0842eaa8c8595d044a3ea406ffdeabc78084643f40d8d7023539311e5 size=205440 wall_s=19 algo=kt134-v4 inputs=66225959715ef6c4 dirty=0 rev=a9460f0c29d4 build=406518d913faa2a8 text=130070 image=/private/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T/vifast-repro-target.mqy8Ay/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

Lệnh chạy (boot ảnh cũ, gửi `cd /bin`, `pwd`, `cd /`, `pwd`, `help`):

```bash
bash /tmp/kt137-boot.sh <ảnh-cũ> /tmp/kt137-ve1-old.log 'cd /bin' 'pwd' 'cd /' 'pwd' 'help'
```

Dòng log **nguyên văn** (`/tmp/kt137-ve1-old.log`):

```text
shell>
# shell: ?
shell>
# shell: ?
shell>
# shell: ?
shell>
# shell: ?
shell>
# shell: help cmds=15
  help | echo <text> | ps | wait <pid> | kill <pid> | ls [path] | cat <path> | write <path> <text> | time | uptime | sync | stat <path> | truncate <path> <size> | jobs | exit
shell>
```

⇒ Trên mã cũ `cd /bin` và `pwd` đều in `# shell: ?` (shell **không biết** lệnh) và `help cmds=15`
— đúng vế "đỏ trên mã cũ". Sha ảnh: `93b8c3b0842eaa8c8595d044a3ea406ffdeabc78084643f40d8d7023539311e5`.

## 5. Vế 2 — **xanh trên mã mới** (ảnh build từ `e179450b`, cây sạch)

```text
REPRO machine=Darwin-arm64 root=/private/tmp/night-L5-kt137 sha256=36fcee017231dc9c6a136312862cd10fa6cb2232704aea9e34dfd28b7aead37e size=209536 wall_s=17 algo=kt134-v4 inputs=170becc54c514926 dirty=0 rev=e179450bf990 build=30bcaa8913cc05e8 text=130070 image=/private/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T/vifast-repro-target.v7MogY/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

Lệnh chạy (harness đầy đủ, một QEMU):

```bash
SHELL_EDGE_LOGDIR=/tmp/kt137-ve2 bash v2/test/boot/shell-edge.sh --image <ảnh-mới>
# exit=0
```

Đầu ra harness **nguyên văn** (`/tmp/kt137-ve2.out`, dòng 64..75 và dòng cuối):

```text
# kt137-cdpwd[pwd]: PASS
# kt137-cdpwd[cd-khong-co]: PASS
# kt137-cdpwd[cd-tep]: PASS
# kt137-cdpwd[cd-thu-tu]: PASS
# kt137-cdpwd[cat-tuong-doi]: PASS
# kt137-cdpwd[stat-tuong-doi]: PASS
# kt137-cdpwd[write-tuong-doi]: PASS
# kt137-cdpwd[cat-rel]: PASS
# kt137-cdpwd[truncate-tuong-doi]: PASS
# kt137-cdpwd[stat-rel]: PASS
# kt137-cdpwd[ls-cwd]: PASS
# kt137-cdpwd: PASS (# shell: pwd /)
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 + new-cmds-edge3 + k7-uu-tien + k7-thu-muc + kt137-cdpwd PASS
```

Log QEMU **nguyên văn** của ca 16 (`/tmp/kt137-ve2/qemu.log`, từ dòng 234 — mỗi dòng là một lệnh):

```text
# shell: pwd /
# shell: cd khong co /khongco
# shell: pwd /
# shell: cd khong phai thu muc /README
# shell: pwd /
# shell: cd /
# shell: cd /
# shell: cd /
# shell: pwd /
# shell: cd /
# shell: pwd /
# shell: cat ok bytes=79 first=ViFastOs
# shell: stat path=/README kind=1 size=79 ino=2
# shell: write /rel.txt bytes=8 sync=ok
# shell: cat ok bytes=8 first=xin chao
# shell: truncate path=/rel.txt size=2
# shell: stat path=/rel.txt kind=1 size=2 ino=5
# shell: ls n=4 path=/
# shell: ls n=4 path=/
# shell: cat ok bytes=79 first=ViFastOs
# shell: cd /
# shell: pwd /
```

Banner của ảnh mới (`/tmp/kt137-ve2/qemu.log` dòng 89):

```text
# shell: up cmds=help/echo/ps/wait/kill/ls/cd/pwd/cat/write/time/uptime/sync/stat/truncate/jobs/exit
```

`help` **không** nằm trong ca 16 (kernel ảnh sản phẩm không tự chạy `help`), nên đo riêng bằng một
lượt boot nữa trên **cùng ảnh** (`/tmp/kt137-ve2-help.log`):

```bash
bash /tmp/kt137-boot.sh <ảnh-mới> /tmp/kt137-ve2-help.log 'help' 'cd /bin' 'pwd' 'cd /' 'pwd'
```

```text
# shell: help cmds=17
  help | echo <text> | ps | wait <pid> | kill <pid> | ls [path] | cd [path] | pwd | cat <path> | write <path> <text> | time | uptime | sync | stat <path> | truncate <path> <size> | jobs | exit
# shell: cd khong co /bin
# shell: pwd /
# shell: cd /
# shell: pwd /
```

Dòng `# shell: cd khong co /bin` là **trung thực** về ảnh sản phẩm hiện tại: FS sản phẩm **không**
có thư mục `/bin` (chỉ seed `/README`), nên kịch bản v1 `cd /bin` của KT-132 **chưa** tái hiện được
— xem §11 (đề nghị task riêng nếu Agent 0 muốn seed thêm thư mục).

**Mười sáu ca cũ** (1..8, 10..15, 8b `truncate-jobs`, `exit`) đều PASS trong cùng lượt chạy — xem
`/tmp/kt137-ve2.out` (mỗi ca một dòng `# <tên>: PASS`). Không ca nào đỏ.

**Tái lập**: build lại ở HEAD `67d2e808` (commit nhật ký **không** đổi build-inputs) cho **cùng**
sha `36fcee017231dc9c6a136312862cd10fa6cb2232704aea9e34dfd28b7aead37e`, `inputs=170becc54c514926`,
`dirty=0` — ảnh **byte-identical**, nên số đo ở trên áp cho cả HEAD cuối.

**T-INFRA (ghi để không đọc nhầm)**: lượt đo chạy lúc máy có **QEMU của sub-agent khác** (ppid=1),
nên harness in thêm hai dòng `# infra after: … qemu_foreign=1 … verdict=KHONG_HOP_LE ly_do=qemu_ppid1=1`
và `# MEASUREMENT_INVALID tag=after ly_do=qemu_ppid1=1`. Đó là **cảnh báo hạ tầng** của
`qemu_guard.sh` (không phải ca kiểm đỏ): phán quyết của harness vẫn `# shell-edge.sh: PASS`,
`exit=0`, và QEMU của chính lượt đo đã được dọn (`qemu_own=0`). Không retry để "lấy xanh" — lượt
chạy lại sau đó (vế 2 lần hai, cùng ảnh) cũng PASS.

## 6. Vế 3 — **đột biến** (bỏ kiểm `kind=2` ⇒ `cd` vào **tệp** vẫn "thành công")

Đột biến: xoá nhánh

```rust
let kind = (reply[0] >> 8) & 0xff;
if kind != KIND_DIR { … return; }
```

trong `cmd_cd` (bản đột biến **không** commit; khôi phục bằng `cp` từ bản tốt rồi build lại — sha
ảnh khôi phục **trùng** sha vế 2, xem dưới).

```text
REPRO machine=Darwin-arm64 root=/private/tmp/night-L5-kt137 sha256=17082ab3c1151346c0639adf0c121f0238e41a60e7567ea51af52b85d39d64c5 size=209536 wall_s=17 algo=kt134-v4 inputs=170becc54c514926 dirty=1 rev=e179450bf990 build=7b0b4d069f4a1df5 text=130070 image=/private/var/folders/jp/05hgrzxj65bgh355l2zs2zz80000gn/T/vifast-repro-target.5OpzTu/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

```bash
SHELL_EDGE_LOGDIR=/tmp/kt137-mut bash v2/test/boot/shell-edge.sh --image <ảnh-đột-biến>
# exit=1
```

Log QEMU **nguyên văn** (`/tmp/kt137-mut/qemu.log`) — `cd /README` **thành công** và `pwd` in
`/README`:

```text
# shell: cd /README
# shell: pwd /README
```

Phán quyết harness **nguyên văn** (`/tmp/kt137-mut.out`, dòng 64..75):

```text
# kt137-cdpwd[pwd]: FAIL (mong đúng 6 dòng '# shell: pwd /', thấy 5)
# kt137-cdpwd[cd-khong-co]: PASS
# kt137-cdpwd[cd-tep]: FAIL (mong đúng 1 dòng '# shell: cd khong phai thu muc /README', thấy 0)
# kt137-cdpwd[cd-thu-tu]: FAIL (chuỗi dòng 'cd' lệch: thấy [# shell: cd khong co /khongco|# shell: cd /README|# shell: cd /|# shell: cd /|# shell: cd /|# shell: cd /|# shell: cd /])
# kt137-cdpwd[cat-tuong-doi]: PASS
# kt137-cdpwd[stat-tuong-doi]: PASS
# kt137-cdpwd[write-tuong-doi]: PASS
# kt137-cdpwd[cat-rel]: PASS
# kt137-cdpwd[truncate-tuong-doi]: PASS
# kt137-cdpwd[stat-rel]: PASS
# kt137-cdpwd[ls-cwd]: PASS
# kt137-cdpwd: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)
# shell-edge.sh: FAIL
```

⇒ Đột biến làm **đỏ đúng ba** phép kiểm (`pwd`, `cd-tep`, `cd-thu-tu`) và **không** làm đỏ mười
phép kiểm còn lại — chứng minh phán quyết **quy đúng lỗi**, không phải "đỏ cả ca". Khôi phục xong
build lại cho **đúng** sha vế 2 (`36fcee01…`, `dirty=0`) — xem §5.

**Đột biến thứ hai (ghi nhận, KHÔNG dùng làm bằng chứng)**: "`pwd` in hằng `/`" **không** phân biệt
được trên ảnh sản phẩm hiện tại, vì FS sản phẩm chỉ có **một** thư mục là gốc `/` (`proc_fs::seed`
chỉ tạo `/README`; shell không có `mkdir`) ⇒ thư mục hiện tại **luôn** là `/`. Vì vậy chọn đột biến
bỏ kiểm `kind` (phân biệt được) và **ghi rõ** ở đây là vế "pwd in hằng" **chưa kiểm** — không tuyên
bố đã chứng minh.

## 7. Ca kiểm + selftest

- Ca mới: **16 `kt137-cdpwd`** trong `v2/test/boot/shell-edge.sh` — 22 lệnh, **11 phép kiểm**, mỗi
  lệnh một dòng phán quyết (`emit_verdicts`). Đặt **trước** (8b) để `/README` còn 79 byte.
- Selftest: mỗi phép kiểm mới có **ca âm** (log sai phải ĐỎ) + log đúng phải XANH; thêm
  `selftest_verdicts` cho ca gộp (log sai ⇒ **2 PASS + 9 FAIL**, mã thoát 1) và hai ca âm **riêng**
  cho hai phép kiểm vẫn xanh trong log sai (`cd-khong-co`, `ls-cwd`).

```bash
bash v2/test/boot/shell-edge.sh --selftest
```

```text
# selftest 16 kt137-cdpwd (sai): ĐÚNG (2 PASS + 9 FAIL, mã thoát 1)
# selftest 16 kt137-cdpwd (dung): ĐÚNG (11 PASS + 0 FAIL, mã thoát 0)
# selftest 16 cd-khong-co: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: cd khong co /khongco', thấy 2)
# selftest 16 cd-khong-co: log ĐÚNG -> XANH đúng
# selftest 16 ls-cwd: log SAI -> ĐỎ đúng (mong đúng 2 dòng '# shell: ls n=<n> path=/' (ls + ls .), thấy 1)
# selftest 16 ls-cwd: log ĐÚNG -> XANH đúng
# shell-edge-selftest: PASS (9/9 ca, 42/42 kiểm: 23 sai→ĐỎ + 19 đúng→XANH)
```

Ảnh test **không hồi quy** (nhánh `cfg(not(shell_fs))` vẫn 6 lệnh) — build lại `ktest-proc` và
kiểm chuỗi trong ảnh:

```bash
cargo build --manifest-path v2/kernel/Cargo.toml -p vifast-kbin-riscv64 \
  --bin vifast-kbin-ktest-proc --target riscv64gc-unknown-none-elf --release --features ktest,user
LC_ALL=C grep -a -o -m1 '# shell: up cmds=[a-z/]*' v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-proc
```

```text
# shell: up cmds=help/echo/ps/wait/kill/exit
```

(`grep -a -c 'shell: pwd'` trên ảnh test = `0` ⇒ không có `cd`/`pwd` trong ảnh test.)

## 8. Cổng `./v2/bin/v2 check`

```bash
./v2/bin/v2 check
```

```text
== v2 check (HEAD e179450b) ==
--- selftest shell-edge (KT-108): OK
v2 check: PASS (L1 chạy trong gate --fast)
```

## 9. Luật 2 — quét lại **toàn bộ** phạm vi danh sách lệnh (CMDS 15 → 17)

Đổi **số lệnh** là đổi phạm vi cổng ⇒ đã quét **mọi** nơi phụ thuộc danh sách/số lệnh:

| nơi | phụ thuộc gì | kết quả |
|---|---|---|
| `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs` | `CMDS`, `help`, banner `# shell: up cmds=` | **đã sửa**: 15 → 17, thêm `cd [path] | pwd` vào `help`, thêm `cd/pwd` vào banner |
| `v2/test/boot/ktest.sh:711-712` | `^# shell: up cmds=help/echo/ps/wait/kill/exit$`, `^# shell: help cmds=6$` | **không đổi** — kỳ vọng của **ảnh test** (không `shell_fs`); đã kiểm bằng chuỗi trong ảnh build lại (§7) |
| `v2/test/boot/expected.txt` | số TAP của 7 ảnh | **không đổi** — tệp chỉ có nhãn + kế hoạch TAP, **không** có danh sách lệnh shell (đã `grep`) |
| `v2/test/boot/shell-edge.sh` | preflight `# shell: up cmds=help(/[a-z]+)+$` (không cứng số) | **không phải sửa** regex; đã thêm ca 16 + selftest, và cập nhật chú thích `9 → 13 → 17` |
| `v2/test/boot/soak.sh:453` | `^# shell: help cmds=[0-9]+$` | **không đổi** — regex không cứng số |
| `spec/v1-parity.toml:70` | `expect = '^# shell: help cmds=[0-9]+$'` | **không đổi** — regex không cứng số |
| `spec/v1-parity.toml` mục `cd`/`pwd` | `status` | **đã sửa**: `deferred-K7` → `active` (**chỉ** `status`, theo ngoại lệ được phép) |
| `scripts/gen-shell-table.py --check` | bảng lệnh trong `docs/11-hien-tai/16-van-hanh.md` | **không đổi** — script đọc `crates/vifast-client/src/main.rs` (**v1**), không đọc shell v2; đã chạy `python3 scripts/gen-shell-table.py --check` ⇒ `bảng lệnh shell khớp mã nguồn` |
| `v2/docs/k6-parity.md` | bảng 28 mục + câu tổng kết "sáu lệnh active / hai mươi hai deferred" | **đã sửa**: hai dòng `cd`/`pwd` → `**active** (KT-137)`; câu tổng kết → mười bốn active / mười bốn deferred (đính chính cả phần KT-130 để lại) |
| `v2/docs/k6-product-shell.md` | tài liệu ảnh sản phẩm | **đã thêm** §11b (KT-137: 15 → 17 lệnh, cwd, đường tương đối, trỏ journal) |
| `v2/docs/k6-product-fs.md:292` | "ảnh test build không `product-fs` ⇒ `help cmds=` giữ nguyên" | **vẫn đúng** — không sửa |
| `v2/agent/reports/K6-CLOSURE.md` | trích log lịch sử (`# shell: up cmds=…`) | **không sửa** — là log đã commit trong khối `scope=`, sửa sẽ làm sai bảng số ↔ khối |
| `v2/agent/DECISIONS.md`, `v2/agent/QUEUE.md`, `v2/agent/STATE.md` | tệp của Agent 0 / lead | **không sửa** (không thuộc vùng ghi của sub-agent) |

## 10. Tệp đã sửa

- `v2/kernel/vifast-kbin-riscv64/init/shell_user.rs` — `Cwd`, `resolve_path`, `cmd_cd`, `cmd_pwd`,
  `fs_dispatch` nhận `cwd`, mọi lệnh đường dẫn phân giải tương đối, `CMDS` 17, `help`, banner.
- `v2/test/boot/shell-edge.sh` — ca 16 `kt137-cdpwd` (11 phép kiểm) + 4 ca selftest mới + chú thích.
- `spec/v1-parity.toml` — **chỉ** `status` của `cd`/`pwd` (`deferred-K7` → `active`).
- `v2/docs/k6-parity.md`, `v2/docs/k6-product-shell.md` — tài liệu theo §9.

## 11. Việc còn lại / cảnh báo cho lead + Agent 0

- **`spec/v1-parity.toml`: `expect` của `cd`/`pwd` vẫn là mô tả**, không phải regex (vì trước đây
  là `deferred-K7`). Nay `status = 'active'` nên `v2/test/parity/run.sh` sẽ chạy hai kịch bản này
  và báo `not ok` (nó `grep -E` trên chuỗi mô tả). **KT-137 chỉ được phép đổi `status`** nên
  **không** sửa `expect`/`send`/`serial_min_count` — cần **Agent 0** sửa (tiền lệ KT-130 đã sửa
  cả bốn trường cho sáu mục). Đề xuất nguyên văn:
  - `cd`: `send = 'cd /\n'`, `expect = '^# shell: cd /$'`, `serial_min_count = 1`
  - `pwd`: `send = 'pwd\n'`, `expect = '^# shell: pwd /$'`, `serial_min_count = 1`
  - và `reason` của hai mục (nay nói "shell v2 CHƯA có builtin cd/pwd") cần cập nhật.
- **FS sản phẩm chỉ có gốc `/`** (`proc_fs::seed` tạo `/README`; shell không có `mkdir`) ⇒ `cd`
  chỉ **đổi được** sang `/`; "lên thư mục cha từ thư mục con" chỉ chứng minh được **từ vựng**
  (`cd /README/..`). Muốn `cd /bin` + `pwd` ⇒ `/bin` như v1 (bằng chứng KT-132) thì phải **seed
  thêm một thư mục** trong `proc_fs.rs` — **ngoài phạm vi KT-137** (đổi nội dung ảnh sản phẩm),
  đề nghị một task riêng nếu Agent 0 muốn.
- **Đột biến "`pwd` in hằng `/`" chưa kiểm** (lý do ở §6) — không tuyên bố đã chứng minh.
- `v2/agent/STATE.md` **không** cập nhật (tệp của lead/Agent 0 trong đêm; nhánh này chỉ commit
  vùng của KT-137).
