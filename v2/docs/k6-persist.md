# K6 — `test-persist` bảy pha trên virtio-blk (KT-75)

Trạng thái: **DONE phần cài + đo trên QEMU thật** (K6, sau KT-74). Ảnh test riêng
**`ktest-persist`**, runner riêng `v2/test/boot/persist.sh` (mười boot, mỗi pha ghi ở **một**
tiến trình QEMU và đọc lại ở tiến trình **khác**). Nợ **D2-111** (“`flush` bền qua reboot”) **ĐÓNG**
bằng số đo trong tệp này, không bằng lập luận.

Quy ước tài liệu (**D2-112**): **mọi số đo nằm trong khối** ```text scope=<tệp-log>``` **và mỗi
dòng là trích nguyên văn** từ đúng tệp đó trong [`agent/JOURNAL/`](../agent/JOURNAL/); ngoài khối
chỉ có **định danh** (mã task, hex, số hiệu spec dạng `x.y`, `file:line`, ngày). Không có
“haystack chung”, không bảng ngoại lệ chứa số đo, và log **mutation/ca âm/đỏ trước** không bao giờ
là nguồn số đo (chúng nằm trong khối `scope-red=`). Script kiểm:
[`JOURNAL/kt75-verify-numbers.sh`](../agent/JOURNAL/kt75-verify-numbers.sh) — chạy lại được, nhận
`KT75_DOCS_ROOT` để trỏ vào **bản copy** (luật **F** của D2-113).

## 1. Phạm vi và nợ

| Nợ / yêu cầu | Trạng thái sau KT-75 |
|---|---|
| D2-111 “`flush` bền qua reboot” | **ĐÓNG** — bảy pha, mỗi pha đọc lại **sau reboot QEMU thật** trên **cùng** ảnh đĩa, so từng byte |
| K6 §10.1 “`test-persist` bảy pha” | **ĐÓNG (phần virtio-blk + FS)** — tạo/ghi đè/append/truncate/rename/xoá+tạo lại/double-indirect |
| D2-108B “bộ đếm luôn bằng không mà không có ca âm thì không chứng minh” | **ÁP** — ca âm `flush` **thật** (§5) + băm/so byte có ca đỏ (§6) |
| D2-113 “ảnh sản phẩm **không** có U-mode/shell” | **CHƯA ĐÓNG** — vẫn là nợ của KT-76 (không thay đổi bởi KT-75) |
| TAP gộp của một ảnh trong `ktest.sh` | **KHÔNG** — chuỗi nhiều boot/ảnh; runner riêng, lý do ở §2.2 |

## 2. Kiến trúc: vì sao **mười** boot, vì sao **kịch bản ở LBA đầu tiên**

### 2.1 Mỗi pha là **hai** tiến trình

Một pha = “ghi rồi **thoát QEMU**” + “boot **lại** QEMU trên **cùng** file ảnh đĩa rồi đọc lại”.
Không có đường nào để dữ liệu của pha trước còn trong RAM của boot sau: tiến trình mới, không gian
địa chỉ mới, cache mô phỏng khởi tạo rỗng. Vì thế pha đọc lại chỉ xanh khi **byte trên đĩa** khớp
payload **sinh lại** từ công thức (D2-106), không phải từ bản sao trong RAM.

### 2.2 Vì sao không nhét vào `ktest.sh`

`ktest.sh` + `expected.txt` giả định **mỗi ảnh một boot** với **một** plan TAP cố định. Chuỗi
persist cần **mười** boot trên **một** ảnh đĩa, mỗi boot một kịch bản khác ⇒ thêm vào `ktest.sh`
sẽ phải nới mô hình “một ảnh một boot” (đúng thứ D2-110 cấm nới). Vì vậy KT-75 giữ **nguyên**
matrix (vẫn là bảy ảnh, boot độc lập) và thêm runner riêng; matrix vẫn xanh **toàn bộ** (số trong khối §8).

### 2.3 Kịch bản ở **LBA đầu tiên**

Runner ghi một bản ghi nhỏ vào **LBA đầu tiên** trước **mỗi** boot (kernel đọc **thô** qua virtio-blk):

- LBA đầu tiên là sector **dành riêng** của FS: `stage()` từ chối LBA đầu tiên, `format` chỉ ghi số không **một** lần,
  và `Fs::initialize` chỉ đọc LBA đầu tiên khi superblock hỏng (`fs.rs:800`) ⇒ sau format nó **không**
  bị FS đọc/ghi.
- Kernel in lại **nguyên văn** kịch bản nó đọc (kể cả `nonce`) ⇒ runner và kernel lệch nhau thì
  `# log-check` đỏ ngay; không có “kịch bản ngầm”.
- Kịch bản chứa: `verify` (pha cần đọc lại), `write` (pha cần ghi), bit `format`, **mode** (bình
  thường / ca âm ghi / ca âm đọc lại), `nonce`.

```text scope=kt75-qemu-b01.log
# persist: plan magic=PL75 ver=1 nonce=1001 verify=0 write=1 flags=1 mode=0 disk_sectors=8192 const:slots=32
# persist: fs mounted=1 formatted=1 replayed=0 capacity=8192 pending_blocks=30
```

## 3. Bảy pha — mỗi pha một hành vi, **ghi ở boot này, đọc lại ở boot khác**

Bảng hành vi (không số): tạo file mới + ghi; ghi đè mở rộng; append tại EOF; truncate về
không rồi ghi lại; rename; xoá rồi tạo lại **cùng tên**; file lớn đi qua **double-indirect**. Kích thước và băm trong từng khối dưới là **số đo**.

Bất biến của pha **ghi** (điều kiện `ok` của TAP): `recorded`, `fsync`, `durable` đều bằng một
(`pending_blocks` bằng không **và** có `T_FLUSH` thật **và** không có `flush` bị bỏ), `overflow`
bằng không (quan sát — chưa có ca âm, xem §3.1). Bất biến của pha **đọc lại**: `match` bằng một,
`mismatched` bằng không, `size_bad` bằng không, `absent_bad` bằng không, `recorded` bằng một,
`pending_blocks` bằng không (`size_bad`/`absent_bad`: quan
sát — chưa có ca âm, xem §3.1).

### 3.1 Bộ đếm — cái nào **đã có ca âm**, cái nào **chưa chứng minh** (D2-108B)

| Bộ đếm | Trạng thái bằng chứng |
|---|---|
| `pending_blocks`, `device_flushes`, `pushed`, `flushes_ignored`, `absent`, `bytes_lost` | **có cặp dương/âm thật**: bảy pha dương vs hai boot ca âm (khối `scope-red=` bên dưới) |
| `match`, `mismatched` | **có ca âm thật**: bộ kiểm đỏ-trước/tự phá sửa payload trên đĩa ⇒ `match` bằng không và `mismatched` khác không |
| `overflow`, `size_bad`, `absent_bad`, `replayed`, `errors`, `id_mismatch`, `timeouts` | **quan sát — CHƯA chứng minh**: chỉ thấy một giá trị ở mọi boot, chưa dựng được ca âm trong đợt này (**nợ**) |
| `recorded`, `fsync`, `durable` | **quan sát — CHƯA chứng minh**: chỉ thấy nhánh xanh; nhánh `durable` bằng không chưa có ca |
| `ok`, `phases`, `records`, `total_ok` | **quan sát — CHƯA chứng minh**: `ok` in **bằng** `phases` (cùng một biến trong `report_total`) nên **không thể** khác — không chứng minh gì; bằng chứng **độc lập** là `writes_ok`/`verifies_ok`/`records` cộng host so **từng byte** `/pstate` |
| `flushes_done` | quan sát; cặp dương/âm đã được phủ bởi `device_flushes` + `flushes_ignored` |

Bảng này là **phép đo**, không phải “bất biến đã chứng minh”: dòng nào ghi *chưa chứng minh* thì
**không** được dùng làm bằng chứng an toàn cho tới khi có ca âm riêng.

```text scope=kt75-qemu-b01.log
# persist: phase=1 action=write wrote=512 recorded=1 fsync=1 durable=1 flush_honored=1 flushes_done=9 flushes_ignored=0 device_flushes=9 pending_blocks=0 pushed=69 overflow=0 detail=0 sha_prefix=06c7090a errors=0
```

```text scope=kt75-qemu-b02.log
# persist: phase=1 action=verify files=1 bytes=512 read=512 absent=0 match=1 sha_prefix=06c7090a mismatched=0 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
# persist: phase=2 action=write wrote=2048 recorded=1 fsync=1 durable=1 flush_honored=1 flushes_done=14 flushes_ignored=0 device_flushes=11 pending_blocks=0 pushed=54 overflow=0 detail=0 sha_prefix=0b537d11 errors=0
```

```text scope=kt75-qemu-b03.log
# persist: phase=2 action=verify files=1 bytes=2048 read=2048 absent=0 match=1 sha_prefix=0b537d11 mismatched=0 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
# persist: phase=3 action=write wrote=1024 recorded=1 fsync=1 durable=1 flush_honored=1 flushes_done=10 flushes_ignored=0 device_flushes=7 pending_blocks=0 pushed=36 overflow=0 detail=2048 sha_prefix=3aad1ae5 errors=0
```

```text scope=kt75-qemu-b04.log
# persist: phase=3 action=verify files=1 bytes=3072 read=3072 absent=0 match=1 sha_prefix=22683ab7 mismatched=0 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
# persist: phase=4 action=write wrote=700 recorded=1 fsync=1 durable=1 flush_honored=1 flushes_done=12 flushes_ignored=0 device_flushes=9 pending_blocks=0 pushed=45 overflow=0 detail=0 sha_prefix=44b066ce errors=0
```

```text scope=kt75-qemu-b05.log
# persist: phase=4 action=verify files=1 bytes=700 read=700 absent=0 match=1 sha_prefix=44b066ce mismatched=0 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
# persist: phase=5 action=write wrote=0 recorded=1 fsync=1 durable=1 flush_honored=1 flushes_done=8 flushes_ignored=0 device_flushes=5 pending_blocks=0 pushed=19 overflow=0 detail=0 sha_prefix=e3b0c442 errors=0
```

```text scope=kt75-qemu-b06.log
# persist: phase=5 action=verify files=1 bytes=700 read=700 absent=1 match=1 sha_prefix=44b066ce mismatched=0 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
# persist: phase=6 action=write wrote=1536 recorded=1 fsync=1 durable=1 flush_honored=1 flushes_done=16 flushes_ignored=0 device_flushes=13 pending_blocks=0 pushed=69 overflow=0 detail=0 sha_prefix=a5785571 errors=0
```

```text scope=kt75-qemu-b07.log
# persist: phase=6 action=verify files=1 bytes=1536 read=1536 absent=1 match=1 sha_prefix=a5785571 mismatched=0 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
# persist: phase=7 action=write wrote=73728 recorded=1 fsync=1 durable=1 flush_honored=1 flushes_done=296 flushes_ignored=0 device_flushes=293 pending_blocks=0 pushed=1879 overflow=0 detail=197568495791 (=single(46)<<32|double(175)) sha_prefix=9cf09c3a errors=0
```

Pha bảy đáng chú ý: `detail` là **cặp block** của inode — single-indirect ở nửa cao,
double-indirect ở nửa thấp — và log in **cả công thức** `(=single(<i>)<<32|double(<j>))` để phép
suy ra kiểm được tại chỗ (D2-106b); cả hai **khác không** ⇒ file lớn **thật sự** đi qua **hai** tầng bảng gián tiếp (không phải “ghi
được file nhỏ rồi gọi là double-indirect”). Cùng tính chất được kiểm độc lập ở host
(`inode_blocks_raw` trong [`tests/kt75_persist_host.rs`](../lib/vifast-fs/tests/kt75_persist_host.rs)).

```text scope=kt75-qemu-b08.log
# persist: phase=7 action=verify files=2 bytes=75264 read=75264 absent=1 match=1 sha_prefix=5fe3d3c0 mismatched=0 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
# persist: phases=7 ok=7 writes_ok=7 verifies_ok=7 records=14 const:phases_want=7 const:records_want=14
# persist: total_ok=1
```

## 4. Nhật ký pha **bền trên đĩa** (`/pstate`) — boot cuối tự đếm, không đếm trong RAM

Mỗi boot append một bản ghi vào `/pstate` (pha, hành vi ghi/đọc lại, `ok`). Boot đọc lại cuối
**đọc lại từ đĩa** rồi mới in `phases=… ok=…`; nếu bất kỳ lần ghi/đọc nào hỏng dấu, `phases` **không**
thể đủ bảy. Đây là chỗ chặn “tự khai xanh”: con số `ok` (bảy) được **suy ra** từ dữ liệu bền, không từ
biến đếm RAM.

## 5. Kiểm chứng **độc lập phía host** (`vifast-fsutil` trên **cùng** ảnh đĩa)

Sau **mỗi** boot, runner chạy `vifast-fsutil fsck` + `ls /` + `stat` + `get` trên **chính** ảnh đĩa
và so **từng byte** với mô hình sinh lại ở [`test/boot/persist_host.py`](../test/boot/persist_host.py)
(cài đặt **độc lập** công thức payload). Ba điều kiện phụ:

- **tập tên** phải khớp: tên cũ sau rename/xoá phải **vắng**, không có entry lạ;
- `fsutil_mutated` bằng không: sha256 ảnh **không đổi** trong lúc kiểm ⇒ kiểm host **không** sửa ảnh
  (chứng minh đây là đọc trên **cùng** file ảnh, không phải “mount rồi ghi đè”);
- host cũng kiểm `/pstate` (nhật ký pha) ⇒ chuỗi append của kernel được xác nhận bằng byte.

```text scope=kt75-host-b07.log
# host file: name=/p5.bin bytes=1536 size_ok=1 sha256=a578557159f28764b58d18a501b1a5e2fcd310842696d253293b6b1f2e3e4b1b want_sha256=a578557159f28764b58d18a501b1a5e2fcd310842696d253293b6b1f2e3e4b1b match=1
# host file: name=/p7.bin bytes=73728 size_ok=1 sha256=9cf09c3a9ef6e9d3a415224e2376a61628e3c6d3692bad93bec5712df3484495 want_sha256=9cf09c3a9ef6e9d3a415224e2376a61628e3c6d3692bad93bec5712df3484495 match=1
# host file: name=/p1.txt absent=1 const:want_absent=1
# host: phase=7 records=13 files=3 fsck=1 fail=0 img_sha256=96496275d2d42f0946514308cb040be43d5c54b07807f48939422ee15c622301 fsutil_mutated=0
```

```text scope=kt75-host-b08.log
# host file: name=/pstate bytes=56 size_ok=1 sha256=0ad5bce5f310e1bfc7030d77e8aa603d756821647542466fe9f2cc2a34193d70 want_sha256=0ad5bce5f310e1bfc7030d77e8aa603d756821647542466fe9f2cc2a34193d70 match=1
# host: phase=7 records=14 files=3 fsck=1 fail=0 img_sha256=45b102d233fcc0a404f471369482ca4592599afb26832d25af63280324c2afba fsutil_mutated=0
```

## 6. Ca âm `flush` — **cắt điện** khi cache chưa được đẩy

### 6.1 Vì sao cần mô phỏng

QEMU ghi thẳng vào file ảnh trên host, **không** có cách nào để nó tự làm mất dữ liệu đã ghi — nếu
chỉ “ghi rồi reboot” thì **không** chứng minh được `flush` có tác dụng. Vì vậy ảnh test đặt một
**cache ghi-bay-hơi** mô phỏng giữa FS và virtio-blk (`PersistDev`): `write` chỉ vào RAM, `flush`
mới **đẩy** block bẩn xuống thiết bị rồi gửi `T_FLUSH` thật. “Cắt điện” = `halt` khi cache còn
block (`pending_blocks` lớn hơn không) ⇒ boot sau không thể thấy dữ liệu, **và** host thấy sha256 ảnh **không
đổi** (không một byte nào ra thiết bị). Đây là **đối chứng**: bảy pha dương đi **cùng** đường ghi,
cùng thiết bị, chỉ khác `flush` có hiệu lực.

Giới hạn phải nói rõ: đây là mô hình **“cache chỉ đẩy khi `FLUSH`”** (chặt hơn đĩa thật, vốn có thể
ghi bất kỳ lúc nào), không phải “đo QEMU làm mất dữ liệu”. Cái được chứng minh là **ranh giới bền**:
không có `FLUSH` ⇒ không có gì ra đĩa; có `FLUSH` ⇒ dữ liệu sống qua reboot (bảy pha dương).

### 6.2 Số đo ca âm

```text scope-red=kt75-qemu-b09.log
# persist: phase=neg action=noflush_write wrote=600 ok=1 flush_honored=0 flushes_ignored=7 device_flushes=0 pending_blocks=12 pushed=0 overflow=0 bytes_lost=6144 (=pending(12)*512)
```

```text scope-red=kt75-qemu-b10.log
# persist: phase=neg action=verify_absent absent=1 p7_match=1 mismatched=0 phases=7 writes_ok=7 verifies_ok=7 records=14 sha_prefix=5fe3d3c0
```

ba trường `device_flushes`, `pushed`, `bytes_lost` (khối trên) là **đo**: pha âm không gửi một `FLUSH` nào và
không đẩy block nào; boot đọc lại thấy `/pneg.txt` **vắng** (`absent` bằng một) trong khi bảy pha trước vẫn
nguyên (`p7_match` bằng một, `records` bằng mười bốn). Host kiểm thêm `changed` bằng không cho **cả hai** boot âm:

```text scope-red=kt75-persist-run.log
# sha: tag=b09 sha=72529ae553c2c8d875b637a837c7b6a4d7828de9e7b465e9293e0bab79bf4ef2 changed=0 (ca âm: không có gì ra thiết bị)
# sha: tag=b10 sha=f552b7059e949bde3852166cb15c090da7e688a601c30e7201a18fc24cc09389 changed=0 (ca âm: không có gì ra thiết bị)
```

## 7. Đỏ trước + chứng minh bộ kiểm **không rỗng**

**Đỏ trước (QEMU thật)**: chạy **đúng** runner với ảnh cũ **không** có persist (`ktest-vblk`) —
boot thật, dòng `TAP version` có mặt, nhưng không có kịch bản/`# persist:` nào ⇒ bộ kiểm **đỏ**:

```text scope-red=kt75-red-before-qemu.log
FAIL log b01.log: dòng plan xuất hiện 0 lần
not ok - b01: log QEMU không khớp mô hình (kind=write phase=1)
FAIL fsutil get /p1.txt: lookup: NotFound
```

**Tự phá có kiểm soát** (`--red-selftest`, chạy trên ảnh **copy** của kịch bản, không sửa cây
thật — luật D2-113F): boot ghi pha một xanh, host ghi đè `/p1.txt` bằng payload **sai** bằng
`vifast-fsutil put-file`, rồi boot đọc lại **phải** đỏ — cả QEMU **và** host:

```text scope-red=kt75-red-selftest.log
# red-selftest: host đã ghi đè /p1.txt bằng 512 B 0xAA (payload SAI)
# red-selftest: TAP QEMU 'not ok 1 - persist verify phase 1' có mặt
# red-selftest: host cũng báo đỏ đúng (fsutil get khác mô hình)
# red-selftest: PASS (QEMU + host đều phát hiện payload bị đổi trên đĩa)
```

**Đỏ trước phía host**: chính bộ kiểm host ở trên là thứ chạy **trước** khi có ảnh (đỏ `NotFound`)
và **sau** khi có ảnh (xanh); ngoài ra
[`tests/kt75_persist_host.rs`](../lib/vifast-fs/tests/kt75_persist_host.rs) có ca **đổi một byte trên
“đĩa” ⇒ đọc lại phải lệch**, và ca âm `flush` ở mức thư viện (cùng thứ tự thao tác, chỉ khác
`flush` có hiệu lực). Ba test host này nằm trong cổng `gate --fast`.

## 8. Cổng, matrix, conformance, soak, clippy, ngân sách

```text scope=kt75-persist-run.log
# persist: phases=7 ok=7 boots=10 neg=1 host_checks=10
# reboot: boots=10 pids=10 phase_reboots=7 (7 pha; mỗi pha ghi ở một tiến trình, đọc lại ở tiến trình KHÁC)
# persist.sh: PASS
```

Matrix giữ **nguyên** bảy ảnh, boot độc lập (ảnh persist do runner riêng điều khiển — §2.2):

```text scope=kt75-ktest-matrix.log
# KTEST: 21/21 boot sạch (smp=matrix, NBOOT=1, ảnh/boot=ktest ktest-guard ktest-user ktest-proc ktest-vblk ktest-con ktest-exec)
# ktest.sh: PASS
```

```text scope=kt75-conformance.log
ok - trace-compare: 61/61 dòng khớp, 5/5 ca (22:16 23:19 24:7 25:7 26:12), build=7304a40491224327
CONF_EXIT=0
```

```text scope=kt75-gate.log
# L1 OK: 1000 seed, 999 trace khác nhau
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
EXIT=0
```

```text scope=kt75-gate-summary.log
TESTS=729 / SUITES=100
```

```text scope=kt75-clippy-kernel.log
# clippy host (v2 workspace, -D warnings): EXIT=0
# clippy vifast-kbin-ktest-persist (features=ktest,vblk): EXIT=0
# clippy vifast-kbin-ktest-persist (all features, -D warnings): EXIT=0
```

Ảnh test **mới** vẫn dưới trần (không nới); ảnh **sản phẩm** đổi **không** byte nào so bản đã soak
(D2-115), và soak lại trên **đúng** sha đó vẫn **không lỗi** (số trong khối dưới):

```text scope=kt75-budget.log
# image=ktest-persist path=/Users/mps/dvchd/ViFastOs/v2/kernel/target/riscv64gc-unknown-none-elf/release/vifast-kbin-ktest-persist file=221248 data+bss=27352 budget=262144 stack=65536 guard=4096 trap_stack=16384 syscall_stack=32768 hart_stacks=831488 noload_hart=950272
```

```text scope=kt75-product-sha.log
BUILD_EXIT=0
# bytes=188552
digest build=ef566cfa583e34bb text=49874
```

```text scope=kt75-soak.log
# soak: 0/1000 lỗi (PASS)
```

## 9. Giới hạn — cái KT-75 **không** claim

- **Không** chứng minh `flush` trên **đĩa cứng thật** hay trên QEMU tự mất dữ liệu: QEMU ghi thẳng
  file ảnh host; ranh giới bền được đo bằng cache mô phỏng **trong** ảnh test (§6.1). Đĩa thật
  (flush semantics của phần cứng) vẫn là nợ của K6-DOD nếu cần.
- **Không** phải test **torn write/ghi dở** giữa transaction — đó là KT-71 (bảy pha crash trên
  journal) và vẫn là bằng chứng riêng; KT-75 kiểm **sau khi commit xong**.
- **Không** có SMP/ngắt: ảnh persist chạy một hart, poll (như driver KT-72).
- **Không** có `fsync` syscall từ U-mode: mọi thao tác đi qua API `vifast-fs` trong kernel test.
- **Nợ ca âm cho bộ đếm**: `overflow`, `size_bad`, `absent_bad`, `replayed`, `errors`,
  `id_mismatch`, `timeouts`, `recorded`/`fsync`/`durable` **chưa** có ca âm trong đợt này (nhãn ở
  §3.1) — chúng là **quan sát**, không phải bằng chứng; dựng ca âm là việc của task sau.
- Matrix **không** chứa ảnh persist (lý do ở §2.2) ⇒ “matrix xanh” **không** phải bằng chứng cho
  persist; bằng chứng là runner riêng ở §8. Đây là chủ ý, không phải thiếu sót.
