# KT-77c · **FS + virtio-blk trong ảnh SẢN PHẨM** (D2-123, mục hai)

**Ngày**: 2026-11-17 · **Task**: KT-77c · **Quyết định**: D2-123 (mục hai và ba), D2-122 (ảnh
canonical + đo từ `artifacts/<sha>.bin`), D2-121, D2-08 (không harness trong ảnh sản phẩm),
A1-175 (target dir riêng), QD §3.5 / D2-72 Q5 (trần `.data+.bss`), D2-102 (in hằng thiết kế),
D2-116 (QEMU guard + đếm orphan).
**Trạng thái**: **đạt** tiêu chí một, ba, năm, sáu của KT-77c; **đạt một phần** tiêu chí hai
(chỉ `ls`/`cat` chuyển `active` — spec **không có** mục `ghi tệp`/`đọc lại sau reboot` riêng,
xem §8); tiêu chí bốn (ma trận + soak) **không** chạy ở đây — D2-123 (mục ba) quy định chạy
**một lần** trên ảnh cuối, sau KT-77c.

Mọi số đo dưới đây trích **nguyên văn** từ log trong `v2/agent/JOURNAL/kt77c-logs/` (OD-K6-5
luật một); văn xuôi dùng số đếm **bằng lời** để không có số ngoài khối.

## 1. Kết luận ngắn

- Ảnh **sản phẩm** build bằng `v2/tools/repro-build.sh` với feature `product-fs`
  (= `product-shell` + nhãn `block` của crate arch, **không** `ktest`): boot tới `shell>`;
  **mount được FS** trên virtio-blk (`-drive`) **hoặc** trên volume RAM khi không có đĩa;
  `ls`, `cat`, `write` chạy; **ghi rồi reboot trên cùng tệp ảnh đĩa đọc lại đúng byte** ở
  một, hai và bốn hart.
- **Tái lập byte**: hai lần build ở **hai** `CARGO_TARGET_DIR` khác nhau, cùng một đường dẫn
  nguồn ⇒ **cùng sha**; ảnh **không** feature vẫn đúng baseline `c1072d72` (KT-79/D2-122).
- **Ngưỡng**: ảnh dưới trần phát hành; `.data+.bss` dưới trần; `size_of::<Machine>()` **không đổi**.
- **Ca âm**: (a) không đĩa hoặc đĩa quá nhỏ ⇒ volume RAM + lý do in ra, **không treo**;
  (b) ghi rồi reboot ⇒ byte bền; (c) ảnh **có shell nhưng tắt FS** ⇒ parity `ls`/`cat` **đỏ**.
- **Bẫy feature** (giống KT-77b): `vblk = ["ktest"]` của crate arch ⇒ ảnh sản phẩm bật
  virtio-blk sẽ kéo `probe`/TAP vào. Nguyên nhân **chỉ là nhãn feature** (không tệp driver nào có
  `cfg(feature = "ktest")`) ⇒ sửa bằng `block = []` + `vblk = ["block", "ktest"]`,
  **không** `BLOCKED-CHO-A0`.

## 2. Ảnh canonical, artifact đo, tái lập

Ảnh đo được **copy** vào `artifacts/<sha256>.bin` (chỉ đọc) **trước** mọi phép đo (A1-175b) và
mọi boot dưới đây boot **từ tệp đó**. Build lần đầu:

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-build-fs-1.log
REPRO machine=Darwin-arm64 root=/Users/mps/dvchd/ViFastOs sha256=64a18ebff24af448a67ddf904cdeb9732091c9a02c8f00aa25cd9e16aa903cb0 size=205440 wall_s=14 build=1e84e92dc17e58ab text=130062 image=/private/tmp/kt77c-final1/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

Lần build **thứ hai**, `CARGO_TARGET_DIR` khác, cùng đường dẫn nguồn ⇒ **cùng sha**:

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-build-fs-2.log
REPRO machine=Darwin-arm64 root=/Users/mps/dvchd/ViFastOs sha256=64a18ebff24af448a67ddf904cdeb9732091c9a02c8f00aa25cd9e16aa903cb0 size=205440 wall_s=15 build=1e84e92dc17e58ab text=130062 image=/private/tmp/kt77c-final2/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

**Ca âm tiêu chí ba** — build **không** feature (đường `# k3 halt`) vẫn đúng baseline:

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-build-baseline.log
REPRO machine=Darwin-arm64 root=/Users/mps/dvchd/ViFastOs sha256=c1072d72d9a50809a2d5f236088b12569b09dc03b0a87d13c113040e9dc4b998 size=82552 wall_s=7 build=2cbe9bbe339d7c02 text=49874 image=/private/tmp/kt77c-base2/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

> **Ghi chú về mốc `c1072d72` và rủi ro tranh chấp**: baseline trên đo **trước** thay đổi của
> **KT-86** (D2-125.3, thêm dòng `# build: rev=<hex> dirty=<cờ>` vào `src/main.rs`). KT-86 đã
> báo trước rằng dòng in đó **đổi sha ảnh không feature**; từ lúc đó `c1072d72` **không còn là mốc**
> cho ảnh sản phẩm — mốc mới do KT-86 ghi ở `v2/docs/k6-build-rev.md`. Ảnh `64a18ebf…` của tài
> liệu này cũng **sẽ đổi sha một lần** khi KT-86 land ⇒ lead phải đo lại trên ảnh **cuối**.
>
> **Ghi chú về tái lập giữa hai đường dẫn nguồn**: hai lần build trên dùng **cùng** đường dẫn
> nguồn (đúng yêu cầu của KT-77c). Tái lập **giữa hai đường dẫn nguồn khác nhau** vẫn **hỏng ở
> phần U-mode** vì `build.rs` gọi `$RUSTC` trực tiếp nên ELF U-mode **không** nhận
> `--remap-path-prefix` của `repro-build.sh` (nợ **A1-184** do KT-77b phát hiện; KT-77c **không**
> sửa — ngoài ngân sách, đã báo lead).
>
> **Ghi chú về số dòng và byte-repro**: một dòng `//` thêm vào **giữa** một tệp nguồn bất kỳ làm
> lệch **số dòng** của mọi panic-location phía sau ⇒ `.rodata` đổi ⇒ baseline đổi sha. Đã đo: thêm
> ba dòng vào `src/main.rs` ⇒ sha khác, `.rodata` lệch **năm** byte; thêm hai dòng vào
> `plat/src/lib.rs` ⇒ vẫn lệch. Cách sửa: hai `#[cfg]` của `plat/src/lib.rs` sửa **tại chỗ** và
> mọi chú thích giải thích đặt ở **cuối tệp**; `mod proc_fs;` cũng khai ở **cuối** `src/main.rs`.
> Đây là quy tắc cần cho mọi task sau: **không chèn dòng vào giữa tệp đang link vào ảnh không
> feature**.

## 3. Ngân sách (tiêu chí bốn)

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-budget.log
# image=product-fs path=/tmp/kt77c-f1/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64 file=205440 data+bss=223696 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
# data+bss-sections: .data=7832 .bss=215864 
```

`size_of::<Machine>()` **không đổi** (hằng `pin` = trần riêng của core):

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-write-smp1.log
# core: product addr=0x80236000 bytes=189384 pin=196608 image_cap=262144 untyped=65536 retype=1 configure=1 suspend=1 resume=1 state_ready=1 objects=3 caps=1 used=0 check_caps=1 check_bytes=1 check_all_bytes=1 check_sched=1 footprint_ok=1
```

`.bss` tăng so với KT-77b vì `Fs` (khoảng **mười ba** KiB) **nằm trong `.bss`**, không trên
stack — xem §5. **Không** nới `MAX_*`/`TOTAL_BLOCKS`/trần nào.

## 4. Hai phương tiện, một giao diện

`src/proc_fs.rs` là **dịch vụ FS phía kernel** của runtime shell sản phẩm (`src/proc_rt.rs`):

- **virtio-blk** — khi QEMU có `-drive` và `-device virtio-blk-pci`: **bền** qua reboot (cùng
  tệp đĩa);
- **volume RAM** — khi không có thiết bị, hoặc đĩa quá nhỏ/không mount được: **bay hơi**
  (log in `volatile=`), không treo.

Quyết định phương tiện **trước** khi mount (`prepare`), rồi format **một lần** nếu superblock
không hợp lệ, rồi mount **một** object `Fs` — mọi lỗi (PCI, thiết bị, format, mount) đều thành
**một dòng log** + reply lỗi cho shell; **không** nhánh nào panic/treo.

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-write-smp1.log
# product fs: medium=virtio-blk blocks=8192 used=2 total=8162 volatile=0 why=ok const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
```

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-nodisk-smp1.log
# product fs: medium=ram blocks=96 used=2 total=66 volatile=1 why=khong-co-pci const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
```

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-disk-too-small-smp1.log
# product fs: medium=ram blocks=96 used=2 total=66 volatile=1 why=dia-qua-nho const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
```

Volume RAM ở PA `0x8300_0000` (ngoài mọi arena của boot) truy cập qua **physmap** bằng hai
wrapper **an toàn** `mmu::read_u64`/`mmu::write_u64` — `src/proc_fs.rs` **không** có `unsafe`.

## 5. `Fs` sống trong `.bss` (không trên stack) — lỗi đã đo và cách sửa

Bản đầu tiên trả `Fs` (khoảng **mười ba** KiB) qua **bốn** tầng (open → open_disk → mount →
empty + sret slot) và **tràn guard page** của khung kernel — đúng lỗi KT-75 đã ghi ở
`ktest_persist.rs:915`:

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-crash-and-lsbug-transcript.log
PANIC: guard page fault scause=0xf stval=0x80400000 sepc=0x8021f864 sstatus=0x8000000200006100
# regs ra=0x80203bb8 sp=0x803fb5d0 gp=0x0 tp=0x0
```

Cách sửa: thêm **một** module nhỏ ở crate arch — `vifast-plat-riscv64/src/static_slot.rs`
(`StaticSlot<T>`, API **an toàn**, "một lần ghi", trả `&'static mut T`) — và `Fs` nằm trong ô
`static` của `proc_rt`; `proc_fs` chỉ chuyển **tham chiếu**. `unsafe` nằm gọn trong crate arch
vì luật R5 (`v2/bin/v2`) chỉ cho `unsafe` ở `kernel/vifast-plat-*`, `boot/`, `vifast-sync`,
`vifast-objmem` — `vifast-kbin-riscv64` **không** thuộc danh sách.

## 6. Đọc lại **sau reboot** trên cùng ảnh đĩa (tiêu chí một)

Boot đầu ghi `/p.txt` = `hello77` rồi `fsync` (commit journal + `T_FLUSH`); QEMU thoát; boot sau
là **tiến trình QEMU mới** trên **cùng tệp đĩa**, **không** ghi gì:

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-write-smp1.log
# product fs: medium=virtio-blk blocks=8192 used=2 total=8162 volatile=0 why=ok const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
# shell: write /p.txt bytes=7 sync=ok
# shell: cat ok bytes=7 first=hello77
# shell: ls n=2 path=/
# product fs: medium=virtio-blk ls=34 cat=4 write=2 sync=2 errors=0 replies=56 dev_reads=94 dev_writes=99 dev_flushes=2
```

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-reboot-read-smp1.log
# product fs: medium=virtio-blk blocks=8192 used=3 total=8162 volatile=0 why=ok const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
# shell: ls n=2 path=/
# shell: cat ok bytes=7 first=hello77
# shell: cat ok bytes=79 first=ViFastOs
# product fs: medium=virtio-blk ls=34 cat=14 write=0 sync=0 errors=0 replies=60 dev_reads=82 dev_writes=0 dev_flushes=0
```

Số `write`/`dev_writes` **bằng không** ở boot sau là **điều kiện của phép đo**: nếu boot đó có
ghi thì "đọc lại" không còn là đọc từ đĩa. Tên tệp trong `ls` là `p.txt` (không phải bản ghi
lỗi của lần đo đầu — xem §10).

Cùng phép đo ở **bốn** hart (hart phụ park, FS chỉ BSP dùng):

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-reboot-read-smp4.log
# smp: boot=2 harts=4 present=0xf online=0xf started=0xb absent=0xf0 timeout=0 dtb=0x87e00000 dtb_magic=1 base=0x80400000 stride=0x1d000
# shell: cat ok bytes=7 first=hello77
```

**Ranh giới bền**: `fsync` = commit journal **rồi** `flush` thiết bị (`virtio_blk::flush` →
`T_FLUSH`). Cơ chế "cắt điện" của KT-75 (cache ghi-bay-hơi: `pending_blocks` khác không lúc
thoát ⇒ mất dữ liệu) là của ảnh test `ktest-persist`; KT-77c **không** dựng lại nó (ngoài
ngân sách). Cái được đo ở đây: **byte trên đĩa host**, thể hiện bằng sha ảnh đĩa **không đổi**
giữa hai boot và `cat` đọc lại đúng byte đã ghi.

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-evidence.log
# evidence: disk_sha256_sau_boot1=cb9ea10368235f0715086cc58a7eafffb6d9bf747f8464299daad138466d91f4
# evidence: disk_sha256_sau_boot2=cb9ea10368235f0715086cc58a7eafffb6d9bf747f8464299daad138466d91f4
# evidence: dia_khong_doi_giua_hai_boot=1 (boot 2 chi doc)
```

## 6b. Ranh giới bền **theo từng thao tác** — và tệp dở sau cắt điện

**Journal của FS là theo TỪNG THAO TÁC, không theo GIAO DỊCH.** Mỗi thao tác ghi (tạo tệp, ghi
block, đổi kích thước, đổi tên) được journal hoá và commit riêng; `fsync` chỉ bảo đảm các thao tác
**đã commit** đã ra thiết bị. Không có khái niệm "một lần ghi tệp là một đơn vị nguyên tử": một lần
ghi nhiều block là **nhiều** thao tác, và cắt điện giữa chừng để lại **tiền tố** của dữ liệu.

Hệ quả, ghim lại làm hành vi đã biết (không phải lỗi):

- Sau cắt điện giữa lúc ghi, trên đĩa có thể còn **tệp dở** — nội dung là tiền tố đúng của payload,
  kích thước nhỏ hơn kích thước đích. `fsck` vẫn **sạch**: journal nhất quán, không có thao tác
  nửa vời.
- **Không mất dữ liệu đã commit.** Mọi thao tác đã commit đọc lại được từng byte sau khi boot lại.
- `fs.create` là **tạo độc quyền**: gặp tên đã tồn tại ⇒ `Exists`. Vì thế một tệp dở để lại sẽ
  **chặn** lần tạo lại cùng tên. Đây là ngữ nghĩa `create` đúng, **giữ nguyên** trong K7 (Agent 0
  chốt: FS nhất quán, không mất dữ liệu đã commit ⇒ đạt C5).
- **Chính sách của người dùng/harness**: gặp tệp dở thì **xoá rồi tạo lại**. `persist.sh --crash`
  làm đúng vậy và có ca kiểm: cắt điện ⇒ dọn tệp dở ⇒ boot lại ⇒ **ghi lại được** pha dở ⇒ chạy
  tiếp tới cuối chuỗi pha, không kẹt. Bỏ bước dọn ⇒ ca kiểm **đỏ** (đột biến đã chạy).

**Nợ v2.1** (ghi để không quên, **không** làm trong K7):

- **Ghi tệp an toàn**: ghi vào tệp tạm rồi **đổi tên nguyên tử** (FS đã có `rename`) ⇒ người dùng
  không bao giờ thấy tệp dở dưới tên thật. Cần quyết định thêm: tệp tạm đặt ở đâu, dọn tệp tạm cũ
  lúc mount thế nào.
- **Quyết định ngữ nghĩa `create`**: giữ `Exists` (như hiện tại) hay thêm cờ "tạo hoặc cắt rỗng"
  (`O_TRUNC`-kiểu). Đổi ngữ nghĩa này ảnh hưởng shell và mọi chương trình dùng FS ⇒ phải qua
  Agent 0, không tự đổi.

## 7. Ca âm (tiêu chí năm) và ca âm harness (tiêu chí sáu)

- **(a) không có đĩa** — boot **không** `-drive`: `medium=ram` + `volatile=` + `why=` lý do;
  `ls`/`cat`/`write` vẫn chạy; **không treo** (shell `exit`, `qemu_rc=` bằng không).
- **(a) đĩa hỏng** — `-drive` trỏ tệp toàn rác: superblock sai ⇒ format lại, `medium=virtio-blk`;
  **không treo**. Số `errors=` khác không ở log dưới là **hai** lần `cat /p.txt` trên volume vừa
  format — đúng đường lỗi, không phải sự cố.
- **(a) đĩa quá nhỏ** — `-drive` trỏ tệp bé hơn vùng metadata tối thiểu: `medium=ram` +
  `why=dia-qua-nho`; **không treo**.
- **(b) ghi rồi reboot** — §6: byte đọc lại **đúng**, sha ảnh đĩa **không đổi** giữa hai boot.
- **(c) tắt feature FS** — build `product-shell` **không** `product-fs`: shell **có** nhưng
  **không** có lệnh FS ⇒ parity `ls`/`cat` **đỏ**, `help`/`echo`/`ps`/`kill` **xanh**.
- **(sáu) marker harness** — quét ảnh sản phẩm + ca âm `ktest-proc`: sản phẩm **không** hit,
  ảnh test **có** hit; probe-gate `ktest=`/`umode=` đúng.

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-disk-broken-smp1.log
# product fs: medium=virtio-blk blocks=8192 used=2 total=8162 volatile=0 why=ok const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
# product fs: medium=virtio-blk ls=34 cat=12 write=0 sync=0 errors=2 replies=58 dev_reads=85 dev_writes=52 dev_flushes=0
```

Log parity ca âm (D2-112c: log ca âm **không** được làm nguồn `scope=`, nên khối dưới dùng
`scope-red=`):

```text scope-red=v2/agent/JOURNAL/kt77c-logs/kt77c-parity-neg-nofs.log
not ok 5 - ls: byte da vao shell (co 8 dong 'shell: ?') nhung expect khong khop sau 4000 ms (^# shell: ls n=[0-9]+ path=/)
not ok 6 - cat: byte da vao shell (co 16 dong 'shell: ?') nhung expect khong khop sau 4000 ms (^# shell: cat ok bytes=7 first=hello77$)
# parity: total=28 ok=4 deferred=22 active=6 (active_da_chay=6)
# parity: KET_LUAN=not_ok (exit 3)
```

Khối dưới là **log quét mới, đo trên chính ảnh cuối** cắt trong `v2/artifacts/` (cả dòng `image=`
để neo sha):

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-harness-scan-final.log
# kt77b-harness-scan: image=v2/artifacts/64a18ebff24af448a67ddf904cdeb9732091c9a02c8f00aa25cd9e16aa903cb0.bin sha256=64a18ebff24af448a67ddf904cdeb9732091c9a02c8f00aa25cd9e16aa903cb0 bytes=205440
# kt77b-harness-scan: markers=15 (chuỗi do mã test phát ra; ảnh canonical đã strip)
# scan product: hits=0
ok 1 - anh san pham khong chua ma harness (hits=0)
# scan negative: hits=25
ok 2 - ca am: anh co harness bi bat (hits=25 > 0)
# feat product: ktest=0 umode=1
# feat harness: ktest=1
ok 3 - probe gate: cau hinh san pham KHONG bat 'ktest' (probe khong duoc bien dich), co 'umode'
ok 4 - ca am cua probe gate: cau hinh anh test BAT 'ktest' (ktest=1 > 0)
# kt77b-harness-scan: KET_LUAN=ok (product=0, negative>0, probe_gate=ok)
```

Log quét **cũ hơn** (đã commit ở lần trước, đo trên ảnh build **trước** hai bản vá cuối — tên tệp
trong reply `ls` và tách `shell_fs`) chỉ còn để **đối chiếu**, không dùng làm nguồn số:
`v2/agent/JOURNAL/kt77c-logs/kt77c-harness-scan.log`.

## 8. Parity `spec/v1-parity.toml` (tiêu chí hai)

Chỉ **hai** mục đổi `deferred-K7` → `active`, `source` giữ nguyên, `send`/`expect` cập nhật theo
shell sản phẩm; **không** thêm/bớt mục nào (tổng số mục giữ nguyên: sáu `active`,
hai mươi hai `deferred-K7`).

- `ls`: `send` = `ls`, `expect` = `^# shell: ls n=[0-9]+ path=/`.
- `cat`: `send` = `write /p.txt hello77` rồi `cat /p.txt`; `expect` neo vào dòng tổng kết của
  `cat` và đòi **đúng chuỗi** `first=hello77`, kèm số byte bất kỳ — gồm **ghi rồi đọc lại trong
  cùng boot** vì spec
  **không có** mục riêng cho "ghi tệp" và một mục chỉ **một** boot nên **không thể** đo
  "đọc lại sau reboot"; phép đó nằm ở §6 (hai boot).

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-parity-smp1.log
ok 5 - ls (khop=8 >= 1, waited_ms=0)
ok 6 - cat (khop=8 >= 1, waited_ms=0)
# parity: total=28 ok=6 deferred=22 active=6 (active_da_chay=6)
# parity: KET_LUAN=ok (exit 0)
```

Cùng bộ mục ở **bốn** hart:

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-parity-smp4.log
# parity: total=28 ok=6 deferred=22 active=6 (active_da_chay=6)
# parity: KET_LUAN=ok (exit 0)
```

`exec`/`runfs`/`run` **giữ** `deferred-K7` (D2-123 mục hai cho phép): `exec` cần nạp ELF từ FS
và `exec` cho U-mode, chưa có trên ảnh sản phẩm; lý do ghi trong mục tương ứng của spec
(không đổi).

## 9. Không hồi quy ảnh test

`shell.elf` **chỉ** có thêm ba lệnh FS khi build với `--cfg shell_fs` (do `build.rs` đặt khi bật
`product-fs`); ảnh test build **không** `product-fs` ⇒ `help cmds=` giữ nguyên và dòng
`# shell: up …` giữ nguyên ⇒ **không** phải sửa kỳ vọng nào của harness (`proc_demo.rs`,
`v2/test/boot/ktest.sh`).

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-regress-ktest-proc.log
1..10
ok 1 - procsrv/shell/con load u-mode
ok 2 - procsrv recv fault exit
ok 3 - shell help echo
ok 4 - shell ps tu core
ok 5 - shell wait ma thoat
ok 6 - procsrv kill treo tcb
ok 7 - shell exit va procsrv finalise
ok 8 - u-mode kernel read fault
ok 9 - faultkind exit
ok 10 - faultkind wx from u
PANIC: u-mode fault scause=0xc stval=0x40002000 sepc=0x40002000 sstatus=0x8000000200006000 from_u=1
```

(PANIC cuối là **W^X chủ đích** của TAP cuối, không phải sự cố.)

## 10. Lỗi đã gặp trong lúc làm (ghi để không tái diễn)

(a) **Tràn khung stack** khi `Fs` đi qua nhiều tầng trả-về-theo-giá-trị — §5.

(b) **Hỏng tên trong `ls`**: reply đặt `ino` ở word mang byte tên rồi OR cả hai ⇒ tệp ino là ba
in thành tên khác (`p` OR ba = `s`). Sửa: `ino` đi trong byte cao của **word không** (dịch sáu
bit) để ba word còn lại **thuần** byte tên. Trước/sau:

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-crash-and-lsbug-transcript.log
  f s.txt
# shell: ls n=2 path=/
```

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-boot-reboot-read-smp1.log
  f p.txt
```

(c) **Byte-repro vỡ vì một dòng chú thích** — §2.

(d) **Lệnh đầu tiên trong payload có thể mất** (nợ K7 của KT-77b: byte đến khi cửa sổ RX đóng
nằm ở FIFO UART): kịch bản bằng chứng lặp mỗi lệnh **hai** lần; parity thì `stdin_wrap.sh` đã
bơm **tám** lần.

## 11. Cái KHÔNG có trong ảnh sản phẩm (trung thực)

- **Không** `exec`/`runfs` trên FS (giữ `deferred-K7`).
- **Không** `mkdir`/`rm`/`sync`/`df`/`fsck` builtin ⇒ các mục đó vẫn `deferred-K7`.
- **Không** dịch vụ FS cho **nhiều** tiến trình: chỉ shell gọi, chỉ **một** `Fs` sống (đúng phạm
  vi "ảnh sản phẩm có shell"); procsrv không giữ FS.
- **Không** chạy ma trận hai mươi bốn boot + soak một nghìn (D2-123 mục ba: chạy **một lần**
  trên ảnh cuối, sau KT-77c).
- **Không** sửa nợ A1-184 (`build.rs` không truyền `--remap-path-prefix` cho ELF U-mode).

