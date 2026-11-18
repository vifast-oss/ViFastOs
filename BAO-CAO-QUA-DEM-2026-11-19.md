# BÁO CÁO QUA ĐÊM — 2026-11-18 19:00 → 2026-11-19

**Người làm**: Agent 1 (lead) · **Phiếu gốc**: quyết định Agent 0 lượt 5 (18/11/2026)
**Tự đủ**: mọi con số dưới đây dán **nguyên văn** từ output công cụ, kèm lệnh đã chạy và loại cây
(luật 1 + luật 3). Bản gốc đầy đủ ở các tệp `v2/agent/JOURNAL/kt14*.md` được trỏ tới từng mục.

> **Đọc trước**: báo cáo này **không** nói "đã chứng minh" ở đâu mà không kèm (i) lệnh, (ii) dòng
> output, (iii) loại cây. Chỗ nào chưa kiểm thì ghi **"chưa kiểm"**.

---

## 0. Bảng tóm tắt

| # | Việc (phiếu lượt 5) | Luồng | Trạng thái | Commit | Ghi chú |
|---|---|---|---|---|---|
| A1 | ĐÍNH CHÍNH R1 lượt 3 + **luật 3** | L1 | **XONG** | `c6a97351` | khối ĐÍNH CHÍNH giữ nguyên văn bản sai bên dưới; luật 1–4 thành tệp riêng |
| A2 | Commit thật của quét QUALIFIERS + R5; câu 1/19 còn bỏ qua | L1 | **XONG** | `c6a97351` | `80601274` → `6d397766`; câu bỏ qua là `k6-persist.md:78`, từ khoá `chưa có` |
| A3 | Vế "đỏ trên **ảnh** mã cũ" cho KT-144 | L1 | **XONG** | `c6a97351` | ảnh `96cc1202…` build từ `5aee1ac7`; ca `write-thieu-noi-dung` **FAIL**, log có `bytes=0` + `size=0` |
| A4 | Siết `algo=` ở **đường đo** | L1 | **XONG** | `c6a97351` | `build-identity.sh` (5 ca) + `algo-strict.sh` (15 ca); bỏ hẳn `--allow-legacy`; 2 đột biến ⇒ đỏ |
| A5a | `StrictBlk` nối **mã dựng request của driver thật** | L2 | **XONG** | `cc60bd22` | `build_request` trong `vifast-virtio` (driver gọi đúng hàm ấy); 11 ca host; 2 đột biến ⇒ `FlushNotNegotiated` / `LengthMismatch` |
| A5b | Quét **toàn repo** bảng tài nguyên | L2 | **XONG** | `cc60bd22` | 5 hằng viết tay ⇒ **dẫn xuất** từ `REGIONS`; 3 `const _` kiểm khớp lúc biên dịch; 15 chỗ còn lại có lý do |
| A6 | `/p7.bin` dở: giữ `fs.create` độc quyền + dọn rồi tạo lại | L3 | **XONG** | `61308b81` | `persist.sh --crash` thêm bước dọn + chạy tiếp tới `phases=7 ok=7`; đột biến bỏ bước dọn ⇒ 3 dòng đỏ; tài liệu FS §6b + nợ v2.1 |
| 4c | `cat`/`stat` lên thư mục, `time` hai tham số | L1 | **XONG** | `61308b81` | đo hành vi thật trên ảnh sản phẩm; ca 15 `k7-thu-muc`; selftest 36/36 |
| N2 | T26 — read-your-own-write ở `MAX_IO=256` | L3 | **CHƯA** | — | xem §8 |
| N3 | T31 — PAGER2 nhận message lặp | L3 | **CHƯA** | — | xem §8 |
| N7 | Audit Q1-A — kênh wake lệch 3 port | L4 | **CHƯA** | — | xem §8 |
| N1 | T33a — treo `x86_64` sau `shell>` | L4 | **CHƯA** | — | xem §8 |
| KT-137 | `cd`/`pwd` + đường tương đối | — | **CHƯA** | — | điều kiện bắt đầu **chưa đủ** (N2/N3/N7 chưa xong) |
| K7 đóng | sáu tiêu chí K6 trên main | — | **CHƯA** | — | điều kiện **chưa đủ** (KT-81 nay đã đóng, nhưng N2/N3/N7 còn) |

**Đếm**: 8/12 việc XONG · 4 việc CHƯA · 0 việc HOÃN · 1 việc CHỜ AGENT 0 (B7 — test `smp` đỏ sẵn).

---

## 1. Kết quả A1–A6

### A1 — ĐÍNH CHÍNH R1 + luật 3

**Sửa**: `PHIEU-TRA-LOI-AGENT0-2026-11-18-LUOT3.md` §1.5 — thêm khối **ĐÍNH CHÍNH** ngay trên dòng
R1, **giữ nguyên văn bản sai** (gạch ngang, không xoá). Tệp mới `v2/agent/LUAT-BAO-CAO-A0.md` chép
đủ **bốn** luật (1–4) + bảng tự kiểm.

**Bằng chứng cho câu "đo lại ở lượt 4"** (dán từ phiếu lượt 4 §1.3, commit `80601274`):

```text
$ git -C . rev-parse --short HEAD
f50c6dea
$ ./v2/bin/v2 gate --fast ; echo "EXIT=$?"
...
EXIT=0
```

Loại cây: **worktree sạch tại `f50c6dea`**. Kết luận ghi trong ĐÍNH CHÍNH: R1 **không** phải hàng
rào có thật ở HEAD; câu "đã chứng minh … trên worktree sạch ở HEAD" ở lượt 3 là **sai**.

### A2 — commit thật + câu 1/19

```text
$ git show --stat --oneline 6d397766 | head -3
6d397766 [KT-142] ba cho Agent 0 bat tra loi + quet toan bo QUALIFIERS + R5 ten nhanh duy nhat + R4/R2 vao JOURNAL
 PHIEU-TRA-LOI-AGENT0-2026-11-18-LUOT3.md           |   10 +
 v2/agent/JOURNAL/kt139-journal.md                  |   30 +-
```

```text
$ git show --stat --oneline 6d397766 | grep -E 'qualifiers|neg-test|scope-table'
 v2/agent/JOURNAL/kt142-quet-qualifiers.md          |  142 ++
 v2/agent/JOURNAL/scope-table-skip-count.txt        |    6 +-
 v2/tools/check-scope-table.py                      |  264 ++-
 v2/tools/git-guard/neg-test.sh                     |   66 +-
```

Loại cây: **worktree sạch tại `6d397766`**. Bảng §0 của báo cáo lượt 4 đã sửa: `80601274` (bằng
chứng lượt 3) → `6d397766` (việc).

**Câu còn bị bỏ qua (1/19)** — nguyên văn từ `kt142-quet-sau.log`:

```text
TONG cau bi bo qua: 1
So tu khoa dang dung: 1 / 8

### 'chưa có' -> 1 cau
    v2/docs/k6-persist.md:78
      nhánh `durable` bằng không chưa có ca |
```

| mục | giá trị |
|---|---|
| vị trí | `v2/docs/k6-persist.md:78` |
| từ khoá loại | `chưa có` (từ khoá **duy nhất** còn dùng trong 8) |
| vì sao loại **đúng** | câu **không** khẳng định số đo — nó nói nhánh `durable` **chưa có ca kiểm**, tức mô tả một **chỗ thiếu**. Không có khối `scope=` nào để so; bắt checker kiểm thì phải bịa khối, đúng thứ luật 1 cấm. |

### A3 — vế "đỏ trên **ảnh** mã cũ" (KT-144)

Build ảnh tại **commit cha** của `a112a214` = `5aee1ac7`, trong worktree riêng:

```text
$ VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" \
    bash v2/tools/repro-build.sh --root /tmp/vifast-old-a112 --target-dir /tmp/vifast-old-a112-target
REPRO machine=Darwin-arm64 root=/private/tmp/vifast-old-a112 sha256=96cc1202c0bfbfe911273798b77cbcada9cc5c42159c77bd20b18c98b1d2b984 size=205440 wall_s=15 algo=kt134-v4 inputs=f636bf7d956cc6b8 dirty=0 rev=5aee1ac7d1f2 build=8f390bafdc1bc984 text=130062 image=/private/tmp/vifast-old-a112-target/riscv64gc-unknown-none-elf/release/vifast-kbin-riscv64
```

Chạy **chính** `shell-edge.sh` của cây hiện tại trên ảnh ấy:

```text
# new-cmds-edge3[write-thieu-noi-dung]: FAIL (mong đúng 1 dòng '# shell: thieu noi dung' (write /a khong noi dung phai BAO LOI), thấy 0)
# new-cmds-edge3: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)
```

Hành vi **thật của mã cũ** trong log QEMU — đúng `bytes=0`, `size=0` như Agent 0 đòi:

```text
192:# shell: write /a bytes=0 sync=ok
194:# shell: stat path=/a kind=1 size=0 ino=3
```

| mục | giá trị |
|---|---|
| commit build | `5aee1ac7d1f2` (cha của `a112a214`) |
| sha256 ảnh | `96cc1202c0bfbfe911273798b77cbcada9cc5c42159c77bd20b18c98b1d2b984` |
| dòng danh tính | `# build: algo=kt134-v4 inputs=f636bf7d956cc6b8 dirty=0` |
| loại cây | **worktree sạch** tại `5aee1ac7` |

Chi tiết: [`v2/agent/JOURNAL/kt144-ve1-anh-ma-cu.md`](v2/agent/JOURNAL/kt144-ve1-anh-ma-cu.md).

### A4 — siết `algo=` ở đường đo

**Hiện trạng trước khi sửa** (quét toàn bộ `v2/`):

```text
$ grep -rn --include=*.sh -E "grep.*# build: (inputs=|rev=|\(inputs\|rev\)=|\[a-z\])" v2/ | grep -v '^v2/agent/JOURNAL/'
v2/test/boot/build-rev.sh:…:        if LC_ALL=C grep -a -q -E '# build: (inputs|rev)=[0-9a-f]+ dirty=[01]' "$image" 2>/dev/null; then
v2/test/boot/shell-edge.sh:1628:    build_line="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=…' -e '# build: [a-z]*=[0-9a-f]* dirty=[01]' "$IMAGE" …)"
v2/tools/pin-artifact.sh:120:        if ! LC_ALL=C grep -a -q -m1 -e '# build: algo=' -e '# build: inputs=' -e '# build: rev=' "$f"; then
v2/tools/pin-artifact.sh:160:LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=…' -e '# build: inputs=…' "$IMAGE" …)"
v2/tools/pre-measure.sh:165:LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=…' -e '# build: inputs=…' -e '# build: rev=…' "$IMAGE" …)"
v2/tools/status.sh:47:    ART_LINE="$(LC_ALL=C grep -a -o -m1 -e '# build: algo=…' -e '# build: inputs=…' -e '# build: rev=…' "$ART" …)"
```

**Sau khi sửa** — chỉ còn **ba** tệp, đúng ranh giới Agent 0 cho phép:

```text
$ bash v2/test/boot/algo-strict.sh
# ok d1 - không tệp nào NGOÀI danh sách được phép còn nhận dạng cũ (3 tệp trong danh sách)
# ok d2 - mọi mục trong danh sách được phép đều còn thật sự khớp mẫu (danh sách không mục rữa)
# algo-strict: 15 PASS, 0 FAIL
# algo-strict.sh: PASS
```

Ba tệp ấy: `status.sh` (hiển thị ảnh lịch sử), `pin-artifact.sh` (ghim ảnh lịch sử),
`build-identity.sh` (**phát hiện** dạng cũ để TỪ CHỐI).

**Vì sao `vbrev_image_identity` KHÔNG nằm trong `build-rev.sh`**: `build-rev.sh` là **định nghĩa**
dấu vân tay, bị `check-algo-pin.sh` ghim (đổi tệp ⇒ phải tăng `VBREV_ALGO`, vì hằng số ấy nằm trong
chuỗi băm). Hàm mới là **người tiêu thụ** định nghĩa, không tham gia phép băm. Đặt nó vào
`build-rev.sh` sẽ buộc tăng `VBREV_ALGO` và làm mọi ảnh/bằng chứng đã ghi lệch mốc **trong khi
thuật toán băm không đổi một byte**. Tách tệp giữ đúng cả hai — và `v2 check` xanh bước
`ghim thuật toán dấu vân tay (KT-134)` mà **không** phải tăng `VBREV_ALGO`.

**Đột biến** (chạy thật):

```text
# not ok a2 - shell-edge.sh NHẬN ảnh phải bị từ chối (reason=algo-mismatch) — [REV-GATE: ACCEPT image=…/badalgo.bin algo=kt000-khong-co inputs=6514ef5cb0fc13a9 dirty=0]
# not ok b2 - soak.sh NHẬN ảnh phải bị từ chối (reason=algo-mismatch) — [REV-GATE: ACCEPT image=…/badalgo.bin algo=kt000-khong-co inputs=6514ef5cb0fc13a9 dirty=0]
# not ok c2 - pre-measure.sh từ chối nhưng SAI reason (mong algo-mismatch) — [# ảnh: sha256=92f8d0cc… size=4157
# algo-strict: 12 PASS, 3 FAIL
# algo-strict.sh: FAIL
```

```text
# not ok d1 - tệp còn nhận dạng cũ mà KHÔNG được phép: v2/tools/pre-measure.sh
# algo-strict: 14 PASS, 1 FAIL
# algo-strict.sh: FAIL
```

Chi tiết: [`v2/agent/JOURNAL/kt145-algo-duong-do.md`](v2/agent/JOURNAL/kt145-algo-duong-do.md).

### A5a — `StrictBlk` nối mã dựng request của driver thật

**Hiện trạng trước khi sửa**: `tests/strict.rs` tự dựng `ReqHeader::new(...)` bằng tay ⇒ chứng minh
**mô hình**, không chứng minh **driver**. Driver thật tự làm **ba** việc rời nhau (header,
`status_index`, độ dài descriptor) — test host chỉ chạm được việc thứ nhất.

**Sửa**: `vifast_virtio::blk::build_request(kind, sector, sectors)` trả **một** `BuiltRequest`
(header đã mã hoá + `sectors` + `data_len` + `status_index`); `virtio_blk.rs::submit` gọi **đúng**
hàm ấy. `data_len` **tính từ số sector**, không nhận từ người gọi.

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-virtio --test driver_request
test doc_mot_sector ... ok
test doc_nhieu_sector ... ok
test doc_qua_mep_bi_tu_choi ... ok
test do_dai_lech_bi_tu_choi ... ok
test driver_that_dung_build_request ... ok
test flush_khi_chua_dam_phan_bi_tu_choi ... ok
test flush_sau_dam_phan ... ok
test ghi_mot_sector ... ok
test hinh_dang_request_dung ... ok
test loai_la_bi_tu_choi ... ok
test sector_cuoi ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Đột biến #1** — tái tạo lỗi v1 #1 (không đàm phán `VIRTIO_BLK_F_FLUSH`):

```text
test flush_sau_dam_phan ... FAILED
  left: Err(FlushNotNegotiated)
test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
```

**Đột biến #2** — tái tạo lỗi v1 #2 (khai độ dài đệm sai):

```text
test doc_mot_sector ... FAILED
  left: Err(LengthMismatch { want: 512, got: 0 })
test doc_nhieu_sector ... FAILED
  left: Err(LengthMismatch { want: 2048, got: 0 })
test result: FAILED. 5 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out
```

Chi tiết: [`v2/agent/JOURNAL/kt81-thiet-bi-gia.md`](v2/agent/JOURNAL/kt81-thiet-bi-gia.md) §4–§10.

### A5b — quét toàn repo bảng tài nguyên

**Năm** hằng viết tay của sáu vùng, nay **dẫn xuất** từ `REGIONS`:

| chỗ | trước | sau |
|---|---|---|
| `pci.rs` `ECAM_BASE` | `0x3000_0000` | `resources::PCI_ECAM.base` |
| `pci.rs` `PCI_MMIO_WINDOW_BASE` | `0x4000_0000` | `resources::PCI_MMIO32.base` |
| `pci.rs` `PCI_MMIO_WINDOW_BYTES` | `1 << 30` | `resources::PCI_MMIO32.size` |
| `pci.rs` `RESERVED_RANGES[0]` | `(UART0_BASE, 0x1000)` | `resources::UART0.base`/`.size` |
| `tests/ipi.rs` | `0x0200_0000` | `resources::CLINT.base` |

**Ba phép kiểm khớp lúc biên dịch** (Agent 0 cho phép "hoặc thêm phép kiểm khớp REGIONS"):

```rust
const _: () = assert!(ECAM_MAP_BYTES <= vifast_plat_common::resources::PCI_ECAM.size);
const _: () = assert!(PCI_MMIO_WINDOW_BYTES == vifast_plat_common::resources::PCI_MMIO32.size);
const _: () = assert!(PCI_MMIO_WINDOW_BASE == vifast_plat_common::resources::PCI_MMIO32.base);
```

**15 chỗ còn lại** đều có lý do ghi trong JOURNAL: **chú thích**, hoặc **địa chỉ ảo của cửa sổ
user** trùng **giá trị** với `PCI_MMIO32` nhưng **khác khái niệm** (`USER_BASE`, `init.ld`,
`inv_user.S`, `ktest_obj.rs`, `ktest_user.rs`, `ktest_capa.rs`, `pager_common.rs`). Dẫn xuất chúng
từ bảng MMIO sẽ tạo **phụ thuộc giả**: đổi cửa sổ MMIO PCIe (hợp lệ) sẽ âm thầm dời cửa sổ user.

**DTB**: repo **không có** tệp `.dts`/`.dtsi`/`.dtb` nào (QEMU `virt` sinh DTB động) ⇒ không áp
dụng. **Linker script**: chỉ đặt `0x80200000` (RAM) và `ALIGN(2M)`; không có địa chỉ MMIO nào.

Chi tiết: [`v2/agent/JOURNAL/kt81-quet-bang-tai-nguyen.md`](v2/agent/JOURNAL/kt81-quet-bang-tai-nguyen.md).

### A6 — `/p7.bin` dở: giữ `fs.create` độc quyền, dọn rồi tạo lại

```text
$ bash v2/test/boot/persist.sh --crash
# crash: dọn — đã xoá /p7.bin DỞ (create là tạo độc quyền; xoá rồi để kernel tạo lại)
# crash: boot sau cắt điện ĐỌC được dữ liệu đã commit (verify pha 6 xanh)
# crash: boot sau cắt điện GHI LẠI được pha 7 sau khi dọn tệp dở (không kẹt)
# crash: boot sau cắt điện ĐỌC LẠI được pha 7 vừa ghi (verify pha 7 xanh)
# crash: chạy tiếp tới CUỐI — tổng kết phases=7 ok=7
# crash: boot sau cắt điện vẫn GHI được (tạo /p1.txt mới, TAP xanh)
# crash: fsck lần cuối sạch
…
# persist.sh: PASS
```

**Đột biến** — bỏ bước dọn:

```text
# crash: dọn — ĐỘT BIẾN: bỏ bước dọn
not ok - crash: boot sau cắt điện KHÔNG ghi lại được pha 7 sau khi dọn tệp dở
not ok - crash: boot sau cắt điện KHÔNG đọc lại được pha 7 vừa ghi (verify pha 7 đỏ)
not ok - crash: KHÔNG chạy tiếp được tới cuối (thiếu '# persist: phases=7 ok=7')
# persist.sh: FAIL (3 lỗi)
```

**Tài liệu FS**: `v2/docs/k6-product-fs.md` §6b mới — journal **theo từng thao tác**, không theo
giao dịch; sau cắt điện có thể còn **tệp dở** (tiền tố đúng, `fsck` sạch, **không** mất dữ liệu đã
commit); `fs.create` độc quyền ⇒ chính sách "xoá rồi tạo lại"; **nợ v2.1** (ghi tạm + `rename`
nguyên tử; quyết định ngữ nghĩa `create`).

Chi tiết: [`v2/agent/JOURNAL/kt146-don-tep-do.md`](v2/agent/JOURNAL/kt146-don-tep-do.md).

### 4c — `cat`/`stat` lên thư mục, `time` hai tham số

Đo **hành vi thật** trên ảnh sản phẩm (`sha256=a4df1ce5…`, `# build: algo=kt134-v4
inputs=7e8bef66f416d40a dirty=0`):

```text
# shell: fs loi code=8
# shell: stat path=/ kind=2 size=512 ino=1
# shell: ?
# shell: cat ok bytes=79 first=ViFastOs
```

| lệnh | hành vi | đánh giá |
|---|---|---|
| `cat /` | `# shell: fs loi code=8` (`IsDir`) | **an toàn** — không in byte nào, không đổi gì |
| `stat /` | `kind=2 size=512 ino=1` | **đúng** — `kind=2` là thư mục |
| `time 5 6` | `# shell: ?` | **an toàn** — `time` chỉ nhận **không** tham số |

⇒ **ghim**, **không** phải hàng chờ Agent 0. Ca 15 `k7-thu-muc` (4 phép kiểm + ca toàn vẹn
`/README`):

```text
# k7-thu-muc[cat-thu-muc]: PASS
# k7-thu-muc[stat-thu-muc]: PASS
# k7-thu-muc[time-hai-tham-so]: PASS
# k7-thu-muc[khong-mat-du-lieu]: PASS
# k7-thu-muc: PASS
# shell-edge.sh: PASS
```

Selftest: `# shell-edge-selftest: PASS (8/8 ca, 36/36 kiểm: 20 sai→ĐỎ + 16 đúng→XANH)`.

Chi tiết: [`v2/agent/JOURNAL/kt147-thu-muc-va-tham-so-thua.md`](v2/agent/JOURNAL/kt147-thu-muc-va-tham-so-thua.md).

---

## 2. Ba vế của từng lỗi K7/N

| lỗi | vế 1 — đỏ trên **ảnh mã cũ** | vế 2 — xanh trên ảnh mã mới | vế 3 — đột biến ⇒ đỏ |
|---|---|---|---|
| **KT-144** `write` thiếu nội dung | ảnh `96cc1202…` (build từ `5aee1ac7`): `new-cmds-edge3[write-thieu-noi-dung]: FAIL`, log `bytes=0` + `size=0` | ảnh `e34334ac…` (build từ `a112a214`): `PASS` | bỏ nhánh `text.is_empty()` ⇒ `write /a bytes=0 sync=ok` + `size=0`, ca `FAIL` |
| **KT-145** `algo=` ở đường đo | **không áp dụng** — đây là **cổng**, không phải lỗi hành vi; vế 1 là "ảnh dạng cũ bị TỪ CHỐI", chạy trên ảnh giả dựng trong `algo-strict.sh` | `15 PASS, 0 FAIL` | bỏ phép so `algo` ⇒ 3 ca đỏ; thêm lại `grep` dạng cũ ⇒ `d1` đỏ |
| **KT-146** tệp dở sau cắt điện | **không áp dụng** — hành vi cũ (pha 7 đỏ) là **đúng ngữ nghĩa**; cái sai là **chính sách harness** | `persist.sh --crash` PASS, `phases=7 ok=7` | bỏ bước dọn ⇒ 3 dòng `not ok` |
| **KT-147** `cat`/`stat` thư mục | **không áp dụng** — hành vi đã đúng; đây là **ghim** | ca 15 PASS trên ảnh sản phẩm | log SAI trong selftest ⇒ 3 phép kiểm đỏ |
| **N2, N3, N7, N1** | **chưa làm** | **chưa làm** | **chưa làm** |

---

## 3. Lịch sử merge trong đêm

**Không có merge nhánh nào.** Lý do: mọi việc trong đêm làm **tuần tự trên `main`** trong một
worktree, không mở nhánh `night/L*`. Đây là **quyết định của Agent 1** (ghi ở §6, mục Q1) vì:

- B2 đòi mỗi luồng một worktree + `CARGO_TARGET_DIR` riêng; nhưng `gate --fast` mất **12–14 phút**
  và **không chạy song song** được với build/cổng khác (AGENTS §6). Bốn luồng × nhiều cổng ⇒ phần
  lớn thời gian là **xếp hàng**, không phải làm việc.
- B2 mục 3 nói rõ `shell-edge.sh`, `soak.sh`, `persist.sh`, `build-rev.sh`, `v2 check`, `gate` là
  **tệp dùng chung**. Trong đêm, **cả bốn** việc A3/A4/A6/4c đều chạm `shell-edge.sh` hoặc
  `persist.sh` ⇒ mở bốn luồng sẽ **tranh cùng tệp**, đúng thứ B2 mục 3 muốn tránh.
- Chạy tuần tự cho **một** lịch sử tuyến tính, không có lần revert nào, không có nhánh dở.

**Thứ tự commit** (mỗi commit đều xanh `v2 check`; `gate --fast` chạy sau khi đủ ba commit):

| # | commit | nội dung | `v2 check` | `gate --fast` |
|---|---|---|---|---|
| 1 | `c6a97351` | KT-145 (A1–A4) | PASS | **PASS** |
| 2 | `cc60bd22` | KT-81 (A5a + A5b) | PASS | **PASS** |
| 3 | `61308b81` | KT-146 + KT-147 (A6 + 4c) | PASS | **PASS** |

**Revert**: **không có lần nào**.

---

## 4. Kết quả quét (luật 2)

| phạm vi quét | vì sao quét lại | kết quả |
|---|---|---|
| **Đường đo nhận dạng cũ** (`v2/**/*.sh`, trừ `v2/agent/JOURNAL/`) | A4 đổi **định nghĩa** "đường đo nhận ảnh nào" ⇒ phải quét **toàn bộ**, không chỉ ba tệp vừa sửa | 6 chỗ → **3 tệp** đúng ranh giới; ca `d1`/`d2` ghim ranh giới ấy |
| **Bảng tài nguyên** (kernel, lib, linker script, DTB, script QEMU, spec) | A5b — Agent 0 đòi quét toàn repo | 20 chỗ: **5** là hằng thật (đã dẫn xuất), **15** là chú thích hoặc VA user (có lý do) |
| **Mã dựng request của driver** | A5a — phải chứng minh driver **dùng chung** mã với test | `driver_that_dung_build_request` (nhúng nguồn driver lúc biên dịch) đòi `build_request(` **có** và `ReqHeader::new(` **không** có |
| **L0 của workspace kernel** | phát hiện ngoài phạm vi khi chạy test | **một test đỏ sẵn** (`smp`) — xem §5 B7 |

---

## 5. Hàng chờ Agent 0

### B7 — test `smp` ĐỎ SẴN và **không cổng nào chạy**

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test smp
---- bo_cuc_per_hart_khong_chong_va_can_trang stdout ----
thread '…' panicked at vifast-plat-common/tests/smp.rs:23:5:
assertion `left == right` failed
  left: 86016
 right: 118784
test result: FAILED. 8 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

- `86016` = `HART_GUARD_BYTES + HART_STACK_BYTES + HART_TRAP_STACK_BYTES` (**thiếu** syscall stack).
- `118784` = `HART_STRIDE`, **đã** gồm `HART_SYSCALL_STACK_BYTES` (thêm ở KT-66).
- Chú thích đầu `smp.rs:10-11` cũng ghi công thức **thiếu** số hạng ấy ⇒ **hai chỗ cùng cũ**.
- `smp.rs` (nguồn **và** test) **không** bị sửa trong đêm ⇒ **đỏ sẵn từ trước**.
- **Không cổng nào chạy**: `v2/Cargo.toml` có `exclude = ["kernel"]` ⇒ `gate --fast` (L0 trên
  `v2/Cargo.toml`) **không** chạm `vifast-plat-common`.

**Hai đường, Agent 0 chọn:**

| | (A) Sửa **test + chú thích** cho khớp `HART_STRIDE` | (B) Sửa **mã** để `HART_STRIDE` bỏ syscall stack |
|---|---|---|
| Đúng hiện trạng | có — linker script đã cấp 32 KiB syscall stack | **không** — hẹp vùng per-hart so với linker script ⇒ `mmu::init` panic |
| Rủi ro | thấp | cao (đổi bố cục bộ nhớ đã chạy) |
| Vướng luật | "sửa kỳ vọng để test xanh" — **cần A0 cho phép rõ** | đổi bố cục bộ nhớ = đổi thiết kế |

**Khuyến nghị của Agent 1**: **(A)**, kèm hai việc bắt buộc — (i) sửa **cả** `smp.rs:23` **và** chú
thích `smp.rs:10-11`; (ii) thêm **cổng** chạy L0 của workspace kernel vào `gate --fast` để lớp lỗi
"test đỏ mà không cổng nào chạy" không tái diễn. Đó là **thêm phép đo**, không nới ngưỡng.

**Trạng thái**: **KHÔNG** sửa trong đêm (B1: Agent 1 được *từ chối* đổi kỳ vọng, không được tự đổi).

---

## 6. Quyết định Agent 1 đã tự làm trong đêm

| # | quyết định | lý do | nằm trong phạm vi B1? |
|---|---|---|---|
| Q1 | **Không** mở bốn luồng song song; làm tuần tự trên `main` | `gate --fast` 12–14 phút và **không** chạy song song (AGENTS §6); cả bốn việc chạm cùng `shell-edge.sh`/`persist.sh` ⇒ B2 mục 3 muốn tránh tranh tệp | có — B1 cho phép "chia luồng"; chọn **một** luồng là cách chia ít rủi ro nhất |
| Q2 | Đặt `vbrev_image_identity` vào **tệp mới** `build-identity.sh`, không vào `build-rev.sh` | tránh tăng `VBREV_ALGO` (sẽ làm lệch mốc mọi ảnh/bằng chứng) trong khi thuật toán băm **không đổi** | có — "vá lỗi, đo thêm, thêm ca kiểm" |
| Q3 | Bỏ hẳn cờ `--allow-legacy` của `pre-measure.sh` | Agent 0 §A4 nói đường đo **phải** từ chối; giữ cờ là giữ đường vòng | có — A0 ra lệnh trực tiếp |
| Q4 | Thêm `--check-image` cho `shell-edge.sh` và `soak.sh` | ca hồi quy A4 cần chạy **chính** hai script ấy mà không mở QEMU (nếu không, ca kiểm phải boot ảnh giả ⇒ treo) | có — "thêm ca kiểm" |
| Q5 | **Không** sửa test `smp` dù nó đỏ | B1: Agent 1 được *từ chối* đổi kỳ vọng, **không** được tự đổi | có — đúng luật |
| Q6 | **Không** dẫn xuất `USER_BASE` từ `PCI_MMIO32` | trùng **giá trị** nhưng khác **khái niệm** (VA user vs PA MMIO); dẫn xuất sẽ tạo phụ thuộc giả | có — "vá lỗi" theo nghĩa **không** tạo lỗi mới |
| Q7 | Gộp KT-146 và KT-147 vào **một** commit `61308b81` | hai task chạm cùng `v2/test/boot/shell-edge.sh`; tách commit sẽ phải tách tệp giữa hai commit ⇒ một trong hai commit **không** tự xanh | có — "merge" là quyền Agent 1 |
| Q8 | **Không** bắt đầu N2/N3/N7/N1 và KT-137 | mỗi lỗi N cần **ba vế** (ảnh cũ + ảnh mới + đột biến) ⇒ mỗi lỗi ≥ 2 chu kỳ `gate --fast`; KT-137 có **điều kiện bắt đầu** là "mọi lỗi N đã merge" (B4) — **chưa đủ** | có — B4 nói rõ điều kiện; B1 cho phép "chuyển việc khác" |

---

## 7. Trạng thái cuối

`./v2/bin/v2 status` — **một lần chạy**, dán nguyên văn:

```text
== dòng OS ViFastOs ==
HEAD        61308b817011 (main)  chưa push: 0
build-inputs 212 mục  dấu vân tay: 7e8bef66f416d40a  dirty: 0
ảnh mới nhất a4df1ce5dfb77866839aca24d724b087eac6e42528ba93ae65dcd98a8b21556c.bin
             sha256=a4df1ce5dfb77866839aca24d724b087eac6e42528ba93ae65dcd98a8b21556c size=205440
             # build: algo=kt134-v4 inputs=7e8bef66f416d40a dirty=0
task        DONE=135 READY=1 BLOCKED=0 REVIEW=0
cổng        ./v2/bin/v2 check | ./v2/bin/v2 check-rules | ./v2/bin/v2 gate --fast
```

**Cổng cuối** (cùng cây, sau commit cuối):

```text
$ ./v2/bin/v2 gate --fast
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
```

Loại cây: **worktree sạch tại `61308b81`** (`git status --porcelain` rỗng trước khi chạy cổng).

**Ảnh đã ghim**: `a4df1ce5dfb77866839aca24d724b087eac6e42528ba93ae65dcd98a8b21556c`, `size=205440`,
`# build: algo=kt134-v4 inputs=7e8bef66f416d40a dirty=0` — dấu vân tay **khớp** cây hiện tại.
(Ảnh `e34334ac…` của `a112a214` vẫn nằm trong `v2/artifacts/` làm mốc lịch sử; nó **không** còn là
"ảnh mới nhất" vì dấu vân tay đã đổi theo KT-81.)

---

## 8. Còn lại + đề xuất thứ tự

| # | việc | vì sao chưa xong | đề xuất |
|---|---|---|---|
| 1 | **N2** (T26, read-your-own-write `MAX_IO=256`) | cần phép đo **50 boot trên `-smp 2`** + ba vế | làm **đầu tiên**: đây là lỗi ưu tiên cao nhất còn mở; phép đo là "đo thêm" nằm trong phạm vi Agent 1 |
| 2 | **N3** (T31, PAGER2 nhận message lặp) | cần **checker "giao đúng một lần"** + ca `clients=4` + đột biến "phát lại Call khi Resume" | làm **thứ hai**: ba thứ ấy đều là **thêm ca kiểm**, không đổi ngữ nghĩa |
| 3 | **N7** (kênh wake lệch 3 port) | cần ca kiểm "một cơ chế wake chung / 3 port" | làm **thứ ba**: crate `vifast-sync` đã có, chỉ thiếu ca |
| 4 | **N1** (T33a, treo `x86_64`) | cần ca "đơn điệu **sau migrate**" + conformance chéo hart | làm **cuối**: nếu không tái hiện được trên riscv64 ⇒ gắn K5' như N5, **không** đóng bằng test giả |
| 5 | **KT-137** (`cd`/`pwd`) | điều kiện bắt đầu (B4) là "4c và mọi lỗi N đã merge" — 4c xong, N **chưa** | sau N2/N3/N7 |
| 6 | **K7 đóng** | điều kiện (B4) là "KT-81 đóng, N2/N3/N7 đóng đủ 3 vế, N1 đóng hoặc gắn K5', 4c + KT-137 xong" — KT-81 **đã đóng**, còn lại **chưa** | sau cùng |
| 7 | **B7** (test `smp` đỏ sẵn) | chờ Agent 0 chọn (A)/(B) | chờ |

**Đề xuất thứ tự phiên tiếp theo**: **B7 (chờ A0) → N2 → N3 → N7 → N1 → KT-137 → đóng K7**.

### 8b. Phát hiện khi chuẩn bị N2 — **"50 boot / 0 đỏ" không đo được như văn bản đang viết**

Trước khi bắt tay N2, Agent 1 đọc lại đúng chỗ mà `v2/docs/kt-t26-analysis.md` §6 ghi là **chưa
làm**: "Chưa chạy trên sim/QEMU (`-smp 2`, 50 boot)". Đọc mã thì thấy **hai** điều khác nhau:

```text
$ sed -n '15,24p' v2/lib/vifast-fs/tests/t26_read_your_own_write.rs
/// Cấu hình tái hiện T26: 256 byte mỗi lần vào/ra (D-04 giữ nguyên, không nới).
const MAX_IO: usize = 256;
const SEEDS: u64 = 1_000;
/// Tự bảo vệ: đổi cấu hình tái hiện là đổi phép đo.
const _: () = assert!(MAX_IO == 256);
```

`MAX_IO = 256` là hằng **cục bộ của test host** — nó mô phỏng một thiết bị chỉ chuyển **256 byte**
mỗi lần. Trên QEMU, `virtio-blk` chuyển theo **sector 512 B** (`SECTOR_BYTES = 512` trong
`virtio_blk.rs`). Nghĩa là:

- **50 boot trên QEMU** đo được đường **boot thật** (kernel + thiết bị thật + reboot), nhưng ở
  granularity **512**, **không** phải 256 — tức **không** tái hiện đúng cấu hình của T26.
- **1.000 seed trên host** đo đúng granularity **256**, nhưng **không** có boot/reboot.

⇒ Hai phép đo **không thay thế nhau**, và văn bản "50 boot / 0 đỏ" hiện tại **không** nói rõ nó đo
cái nào. Agent 1 **không** tự chọn: đây là chỗ định nghĩa phép đo thay đổi ⇒ **hàng chờ Agent 0**.

**Ba đường, Agent 0 chọn:**

| | (A) 50 boot QEMU `-smp 2` ở 512 B | (B) 50 **lần chạy** test host 256 B | (C) Thiết bị giả giới hạn 256 B trong kernel |
|---|---|---|---|
| Đo được | boot/reboot thật, granularity **sai** | granularity **đúng**, không boot | **cả hai** |
| Công sức | ~3 phút | ~1 phút | phải thêm đường chạy vào kernel (đổi sản phẩm) |
| Rủi ro | kết luận "0 đỏ" **không** nói gì về T26 | không nói gì về boot | đổi bề mặt sản phẩm ⇒ cần A0 |

**Khuyến nghị của Agent 1**: **(A) + (B) cùng lúc**, và ghi rõ trong tài liệu rằng **hai** phép đo
cùng tồn tại vì chúng đo **hai** thứ khác nhau; **(C)** để v2.1. Nếu Agent 0 muốn **một** con số duy
nhất thì phải nói rõ con số ấy đo cái gì — nếu không, "0/50" sẽ lại là một con số **đúng nhưng
không chứng minh điều người đọc tưởng**.
