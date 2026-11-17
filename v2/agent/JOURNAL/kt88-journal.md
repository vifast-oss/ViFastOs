# KT-88 — LẦN ĐO CUỐI CỦA K6 (D2-123.3): một lần, trên ảnh sản phẩm **cuối**, đo từ `artifacts/<sha256>.bin`

**Agent 2 dòng OS · phiên mới · ngân sách 3 h · 2026-11-17**

Nguồn sự thật của mọi số dưới đây là **log thô** trong `v2/agent/JOURNAL/kt88-logs/` (mỗi dòng ghi
`file:dòng`). Không có số nào chép tay từ tài liệu.

---

## 0. Mốc đo

| Mục | Giá trị | Log nguồn |
|---|---|---|
| HEAD lúc đo (đóng băng) | `cf96eb9b589d526b30d839e301e81bd0f8b1bd13` (`cf96eb9b589d`) | `kt88-22-freeze-run2.log:3-5` |
| `dirty` (phạm vi `v2/kernel v2/lib`) | **0** | `kt88-22-freeze-run2.log:6` |
| Ảnh sản phẩm **cuối** | `v2/artifacts/291502d25c50b4e74476f08663bbe915d6b96f3d9961c66d9fb3f80d5a250353.bin` | `kt88-22-freeze-run2.log:28` |
| sha256 | `291502d25c50b4e74476f08663bbe915d6b96f3d9961c66d9fb3f80d5a250353` | `:29` |
| size | **205.440 B** | `:29`, `:131` |
| `build=` / `text=` (D2-80, FNV-1a 64 trên `.text`) | `build=1e84e92dc17e58ab text=130062` | `:30` |
| `rev` / `dirty` nhúng trong ảnh | `rev=cf96eb9b589d dirty=0` | `:31` (và dòng 68 của mỗi `boot-final-smp*.log`) |
| QEMU | 11.1.3 (sàn 9.2) · OpenSBI `fw_dynamic` | `:159`, `:284` |
| `qemu_con_sot_total` | trước **0** → sau **0** (đo lại sau khi QEMU thoát hết) | `:6`, kiểm riêng `pgrep` = 0 |

**Hai lần build ở hai `CARGO_TARGET_DIR` khác nhau ⇒ CÙNG sha** (yêu cầu bước 1):

| Lần | `CARGO_TARGET_DIR` | sha256 | rev | dirty | wall |
|---|---|---|---|---|---|
| 1 | `/tmp/kt88-frz-1` | `291502d2…0353` | `cf96eb9b589d` | 0 | 13 s (`:14`) |
| 2 | `/tmp/kt88-frz-2` | `291502d2…0353` | `cf96eb9b589d` | 0 | 14 s (`:22`) |

`CUNG_SHA=1`, `cmp=identical` (`:26-27`). HEAD được kiểm **trước và sau từng lần build**
(`:15`, `:23` — cùng `cf96eb9b…`), nên sha không phải "trùng may mắn qua hai HEAD khác nhau".

---

## 1. Bảng 6 tiêu chí (D2-123.3)

Quy ước D2-102: `const:` = hằng in từ `const` trong mã; công thức ghi rõ cho giá trị suy ra.

| # | Tiêu chí | Số đo | Log nguồn (dòng in ra số) | Kết |
|---|---|---|---|---|
| 1 | Boot ảnh sản phẩm cuối ở **smp 1/2/4** ⇒ tới `shell>` | 3/3: `# shell: up cmds=help/echo/ps/wait/kill/ls/cat/write/exit`; `# product shell: harts=1/2/4`; `# smp: boot=… harts=1/2/4 … timeout=0`; `shell>` xuất hiện; `# k3 shell: handoff product-shell=1` | `boot-final-smp1.log:68,80,85,89` · `boot-final-smp2.log:68,80,85,89` · `boot-final-smp4.log:68,80,85,89`; `REV-GATE: ACCEPT` tại `kt88-22-freeze-run2.log:41,48,55` | **ĐẠT** |
| 2 | `parity/run.sh --mode serial` trên ảnh cuối ở **smp 1 và 4** | **6/6 ok** cả hai (`help/echo/ps/kill/ls/cat`), `not_ok=0`, `active=6`, `deferred=22`, exit 0 ×2 | `kt88-22-freeze-run2.log:67-77` (smp1 oks), `:78-83` (tổng + exit 0), `:101-112` (smp4 oks), `:113-118` | **ĐẠT** |
| 3 | Ca âm: (a) không shell ⇒ `khong_co_shell` + mã thoát riêng; (b) không FS ⇒ `ls`/`cat` đỏ, 4 lệnh kia xanh | (a) **exit 5**, `khong_co_shell=6`, `not ok 1..6`; (b) `ok 1..4` (help/echo/ps/kill), `not ok 5 ls`, `not ok 6 cat`, **exit 3**, `ok=4 not_ok=2` | (a) `kt88-10b-negatives-frozen.log:29-39,41,44-45`; (b) cùng tệp `:81-89,92-93` | **ĐẠT** |
| 4a | **MA TRẬN** `ktest.sh` 7 ảnh × smp 1/2/4 | **21/21** boot sạch (`# KTEST: 21/21 boot sạch`), `ktest.sh: PASS`, 0 FAIL, 0 TIMEOUT | `kt88-22-freeze-run2.log:256-278` (`:278` là dòng tổng), `:279` PASS, `:280` exit 0 | **ĐẠT** |
| 4b | **+ ảnh sản phẩm 3 cấu hình** | 3/3 (smp 1/2/4) — chính là 3 boot của tiêu chí 1 ⇒ **24/24 boot** cho tiêu chí 4 (công thức: 21 + 3) | `boot-final-smp{1,2,4}.log`; đếm ở `kt88-22-freeze-run2.log:41,48,55` | **ĐẠT** |
| 4c | **SOAK** `soak.sh 1000 3` | **0 lỗi/1000** (`ok=1000 fail=0 tổng=1000/1000`), `soak.sh: PASS`, wall 238 s | `kt88-22-freeze-run2.log:1304-1308` | **ĐẠT** |
| 5 | Ngưỡng | ảnh **205.440** ≤ 262.144; `.data+.bss` **223.696** ≤ 262.144; `size_of::<Machine>()` **189.384** pin **196.608** | ảnh: `:130-131`; `.data+.bss`: `:131-134` (`budget_exit=0`); Machine: `ktest-matrix/ktest-smp1-1.log:253` = `# core: link … bytes=189384 pin=196608 … sum_ok=1 in_image=1`, và `ktest-user-smp1-1.log` `# user core: static=189384 pin=196608` | **ĐẠT** |
| 6 | Harness trên ảnh cuối | sản phẩm **0 hit**; ca âm `ktest-proc` **22 hit**; probe-gate **`ktest=0 umode=1`** | `kt88-22-freeze-run2.log:137-154` (`:139` hits=0, `:146` hits=22, `:149` `ktest=0 umode=1`, `:153` KET_LUAN=ok) | **ĐẠT** |

---

## 2. Đọc lại tệp sau reboot (bước 5) — hai boot trên **cùng một tệp đĩa**

Ảnh cuối `291502d2…`, `-drive file=kt88-final.img,format=raw` + `-device virtio-blk-pci`:

| Boot | Lệnh gửi | Bằng chứng | Log |
|---|---|---|---|
| 1 | `write /p.txt hello77` + `exit` | `# shell: write /p.txt bytes=7 sync=ok`; `# product fs: medium=virtio-blk … write=1 sync=1 … dev_writes=83 dev_flushes=1` | `persist-2boot-final/boot1.log`; dòng tổng ở `kt88-22-freeze-run2.log:1427-1428` |
| 2 | `cat /p.txt` + `exit` | `# shell: cat ok bytes=7 first=hello77`; `# product fs: medium=virtio-blk … cat=2 **write=0** sync=0 … dev_writes=0 dev_flushes=0` | `persist-2boot-final/boot2.log`; dòng tổng `kt88-22-freeze-run2.log:1431-1432` |

- sha256 **tệp đĩa không đổi** giữa hai boot: `48ece16b600a657de4981b8f4d885ddb8844ede35c2b54674fbcbbc07034023d`
  ở cả hai (`kt88-22-freeze-run2.log:1425`, `:1429`; `disk_sha_khong_doi=1` `:1433`).
- Boot 2 **không ghi** gì: `write=0`, `dev_writes=0`, `dev_flushes=0` (`:1432`).
- Cổng rev trên log boot 2: `REV-GATE: ACCEPT rev=cf96eb9b589d …` (`:1435`).

## 2b. Persist 7 pha (runner `persist.sh`, 10 boot, mỗi pha ghi ở tiến trình này đọc ở tiến trình **khác**)

`# persist: phases=7 ok=7 boots=10 neg=1 host_checks=10` (`kt88-22-freeze-run2.log:1396`),
`# persist.sh: PASS` (`:1421`), `phase_reboot … distinct_process=1` cho cả 7 pha (`:1414-1420`),
host-side `fsck=1 fail=0 fsutil_mutated=0` mọi pha (`:1325`…`:1395`).

---

## 3. Ca âm — baseline MỚI sau KT-86

### 3(a) Không feature shell (canonical, `repro-build.sh` **không** `CARGO_TARGET_DIR` trong cây)

| Trường | Giá trị | Log |
|---|---|---|
| sha256 | `276e68c883147fed2f62a6814132e25fe79fd10ee22d8f73ba6d8349143ce5d6` | `kt88-10b-negatives-frozen.log:12,14` |
| size | **82.552** | `:14` |
| `build=` / `text=` | `build=5bed09c031a901fd text=49904` | `:14` |
| `rev` / `dirty` | `cf96eb9b589d` / 0 | `:12,15` |
| artifact | `v2/artifacts/276e68c8….bin` | `:16` |
| parity | **exit 5** = `khong_co_shell`, `not ok 1..6` (`khong thay shell trong log`), `not_ok=6 khong_co_shell=6` | `:29-39` (`not ok`), `:41` (`not_ok=6 khong_co_shell=6`), `:44-45` (`KET_LUAN=khong_co_shell (exit 5)`, `NOSHELL_PARITY_EXIT=5`) |

> Baseline **cũ** `c1072d72…` (80,6 KiB) build **trước** KT-86 ⇒ không có dòng `# build:` ⇒ chỉ còn
> **giá trị lịch sử**, không dùng làm mốc so được nữa. Baseline mới ở trên có `dirty=0`, `rev` khớp
> HEAD đóng băng, cùng bộ cờ canonical (remap + strip + rustc-canon) ⇒ so được với ảnh cuối.

### 3(b) Có shell, **không** FS (`--features product-shell`)

| Trường | Giá trị | Log |
|---|---|---|
| sha256 | `21b08f7faf73de963efd30d06dc6533155128b3b4118257c874c0e14c8440d23` | `kt88-10b-negatives-frozen.log:63,65` |
| size | **176.768** | `:65` |
| `build=` / `text=` | `build=40084955930e6bd5 text=106922` | `:65` |
| parity (smp 1) | `ok 1..4` = help/echo/ps/kill; `not ok 5 - ls`; `not ok 6 - cat`; **exit 3**; `total=28 ok=4 not_ok=2` | `:81-87` (4 ok + 2 not ok), `:88-89` (`ok=4 not_ok=2`), `:92-93` (`KET_LUAN=not_ok (exit 3)`, `SHELLNOFS_PARITY_EXIT=3`) |

---

## 4. Điều đã xảy ra với HEAD (bằng chứng thực nghiệm cho B5 của lead)

Trong ~20 phút, `main` trôi **4 lần** (3 commit dòng ViScript `[ST-174]`/`[ST-174b]`/`[ST-160q]`
+ 1 commit sổ sách `[KT-00]` của lead). Cổng KT-86 so `rev` nhúng trong ảnh với **HEAD hiện tại**
nên **cả bốn** lần đều `REFUSE`, dù `git diff <rev>..HEAD -- v2/kernel v2/lib` **rỗng**:

- `kt88-04-shell-parity.log:93` `REV-GATE: REFUSE reason=rev-mismatch image_rev=d40f1e63a61d head_rev=4e903d94c12f`
- `kt88-04-shell-parity.log:143` `… image_rev=d40f1e63a61d head_rev=ffe75569e0b2`
- `kt88-21-freeze-run.log:21` build 2 ở HEAD **khác** build 1 (rev `cf96eb9b589d` so với `ffe75569e0b2`)
  ⇒ sha khác nhau **chỉ vì chuỗi rev**, `build=`/`text=` y hệt (`1e84e92dc17e58ab`/`130062`).

Ảnh `38714b79…` (rev `d40f1e63a61d`, size 205.440, `build=1e84e92dc17e58ab text=130062`) **vẫn
đúng mã kernel** — chứng minh: `git diff d40f1e63..HEAD -- v2/kernel v2/lib` rỗng. Ảnh này **giữ
lại** làm bằng chứng cho B5; nó **không** được dùng làm số báo cáo (mọi số ở §1 lấy từ ảnh
`291502d2…` build ở HEAD đóng băng `cf96eb9b589d`).

Không lần nào tôi tắt cổng, dùng `--no-verify`, hay chạy lại để lấy xanh.

---

## 5. Phần **KHÔNG** đạt / hạn chế phải nói thẳng

1. **Không có tiêu chí nào đỏ**, nhưng có **ba hạn chế thật**:
   - **`v2/test/parity/run.sh` KHÔNG áp cổng danh tính KT-86** (khác bốn runner boot). Cổng cho
     parity do tôi **gọi tay** (`vbrev_gate`) và ghi `REV-GATE: ACCEPT` — nếu ai đó chạy parity mà
     không gọi cổng, parity vẫn cho ra số trên ảnh lệch nguồn. Đây là lỗ hổng của harness, không
     phải của phép đo này.
   - **Ma trận/soak/persist đo ảnh do runner tự build** ở `v2/kernel/target/…/release/` (plain
     `cargo build`, không phải pipeline canonical) — **không** đo trực tiếp `artifacts/<sha>.bin`.
     Ảnh canonical được đo trực tiếp ở tiêu chí 1, 2, 5, 6 và §2. Cổng rev chứng minh cả hai cùng
     một HEAD, nhưng đây là **hai tệp khác nhau** (soak: `9dfd2ca9…`, `build=da8700da08125add`;
     canonical: `291502d2…`, `build=1e84e92dc17e58ab`). Muốn "một ảnh cho mọi phép đo" thì phải
     sửa runner (ngoài quyền KT-88).
   - **Harness scan lần 1 đỏ vì PATH thiếu `~/.cargo/bin`** (`cargo: command not found` ⇒ mọi bộ
     đếm feature-graph = 0 ⇒ `not ok 3,4`). Đây là **T-INFRA của phép đo**, tôi chạy lại **một lần**
     với PATH đúng (không sửa script, không đổi ngưỡng): `kt88-12b-harness-scan-pathfix.log`.
     Log đỏ **giữ nguyên** (`kt88-12-harness-scan.log`).
2. **`vbrev_gate` quá chặt so với ý định** (B5): đã có bằng chứng §4. Đề xuất (không tự làm):
   chấp nhận khi `image_rev` là tổ tiên của HEAD **và** `git diff image_rev..HEAD -- v2/kernel v2/lib`
   rỗng.
3. **KT-86 làm mọi baseline cũ không so được**: `c1072d72…` (không shell) và `07c34fc6…` (shell,
   KT-77b) đều **trước** KT-86 ⇒ không có `# build:` ⇒ cổng từ chối. Baseline mới: §3.

---

## 6. Thời gian

| Mốc | Giờ (JST) |
|---|---|
| bắt đầu (đọc trạng thái, kiểm HEAD/dirty) | 14:57 |
| build ảnh đầu (HEAD `d40f1e63`) | 15:00 |
| parity 6/6 ở smp 1/2/4 trên ảnh `38714b79` | 15:01–15:15 |
| phát hiện HEAD trôi, dừng ở ranh giới, báo lead | 15:06 |
| ca âm (bản đầu) + harness scan + đọc lại sau reboot | 15:15–15:25 |
| chờ đóng băng HEAD | 15:25–15:29 |
| **chạy toàn bộ trên HEAD đóng băng `cf96eb9b589d`** | 15:29–15:45 |
| ca âm dựng lại ở HEAD đóng băng + viết báo cáo | 15:46–15:57 |
| **tổng** | **~60 phút** (ngân sách 180) |

Phần lớn thời gian chờ là **đóng băng HEAD** (điều kiện tiên quyết của cổng), không phải thời gian đo:
riêng chuỗi đo đầy đủ (build 2×, 24 boot, soak 1000, persist 10 boot, 3 ca âm) chạy **16 phút**.
