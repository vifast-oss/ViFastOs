# KT-77b · Ảnh **sản phẩm** có U-mode + shell (OD-K6-2)

**Ngày**: 2026-11-17 · **Task**: KT-77b · **Quyết định**: OD-K6-1, OD-K6-2, D2-122 (ảnh canonical),
D2-08 (không harness trong ảnh sản phẩm), D2-119.
**Trạng thái**: **đạt tiêu chí một, ba, năm, sáu** của OD-K6-2; **tiêu chí hai CHƯA ĐẠT** (mới chạy được phần `active`
của `spec/v1-parity.toml`, bốn lệnh) — phần "liệt kê thư mục / đọc tệp / ghi tệp / đọc lại sau
reboot / chạy chương trình" đang là `deferred-K7` **trong spec đã đóng băng**, nên **không** được
tính là đạt trọn tiêu chí hai; **tiêu chí bốn** (ma trận + soak) thuộc bước đo của lead, tài liệu
này chỉ cung cấp log boot một/hai/bốn hart.

Mọi số đo dưới đây nằm trong khối ```` ```text scope=… ```` trích **nguyên văn** từ log đã commit
(OD-K6-5 luật một); chữ trong văn xuôi dùng số đếm bằng lời để không có số ngoài khối.

## 1. Kết luận ngắn

- Ảnh **sản phẩm canonical** build bằng `v2/tools/repro-build.sh` với feature `product-shell`:
  boot tới `shell>` trên QEMU riscv64 ở **một, hai, bốn hart**; dòng `# shell: up` do **shell
  U-mode** in (không phải kernel).
- Kịch bản **bốn lệnh `active`** (`help`, `echo`, `ps`, `kill`) chạy qua **serial ngoài** trên
  ảnh sản phẩm: **bốn trên bốn** ở mỗi cấu hình hart, `KET_LUAN=ok`.
- **Ca âm shell tắt**: build canonical **không** feature ⇒ runner báo `KET_LUAN=khong_co_shell`
  (`exit` năm) — đây là ca âm bắt buộc để tiêu chí một được tính.
- **Ca âm detector**: ảnh sản phẩm **không** có marker harness; ảnh test (`ktest-proc`) bị bắt.
- **Ca âm thứ ba**: gửi byte nhưng `expect` sai ⇒ byte **đã vào shell** (`# shell: ?`) và kịch bản
  đỏ (`exit` ba) — chứng minh kịch bản không phải "xanh sẵn".
- Ngân sách: ảnh canonical **dưới trần**; `size_of::<Machine>()` **không đổi**.
- **Sha phải sinh từ cây đã commit** (D2-108A lần thứ tư): ảnh bằng chứng dưới đây build từ
  commit `12fbff3e` khi `v2/kernel` sạch; bản `39ca3cc6…` là **bản trước commit — KHÔNG dùng**
  (không bằng chứng, không matrix/soak).
- **Detector có lớp `probe`** (D2-123(1)): ảnh sản phẩm không bật feature `ktest` ⇒ không biên
  dịch `probe`; ca âm dùng **feature-graph** vì ảnh canonical đã strip hết symtab.
- **Đường U-mode trước KT-77b phụ thuộc feature `ktest`** của crate arch (xem §9): đã tách bằng
  feature `umode` (kernel) + `product-shell` (ảnh sản phẩm); ảnh sản phẩm **không** link module
  harness nào. Đây **không** phải "nhét harness vào ảnh sản phẩm" — hướng ngược lại.

## 2. Ảnh đo: sinh từ **cây đã commit** (D2-108A)

**Luật build (lần thứ tư của D2-108A):** sha trong tài liệu **phải** sinh từ **cây đã commit**;
quy trình: `build → commit → build lại xác nhận cùng sha`. Bản `39ca3cc6…` được build khi
`proc_rt.rs` còn khác bản đã commit ⇒ **gắn nhãn "bản trước commit"**: KHÔNG dùng làm bằng chứng,
KHÔNG dùng cho matrix/soak.

Ảnh bằng chứng build từ commit `12fbff3e` (cây `v2/kernel` sạch — đã đối chiếu `git status`
trống trước khi build). Sha + số byte do **runner đọc lại từ chính tệp đang boot**, không phải số
chép tay:

```text scope=v2/agent/JOURNAL/kt77b-logs/kt77b-parity-smp124.log
===== SMP=1 =====
ok 1 - help (khop=16 >= 2, waited_ms=0)
ok 2 - echo (khop=8 >= 1, waited_ms=0)
ok 3 - ps (khop=8 >= 2, waited_ms=0)
ok 4 - kill (khop=8 >= 1, waited_ms=0)
# parity: KET_LUAN=ok (exit 0)
===== SMP=2 =====
ok 1 - help (khop=16 >= 2, waited_ms=0)
ok 2 - echo (khop=8 >= 1, waited_ms=0)
ok 3 - ps (khop=8 >= 2, waited_ms=0)
ok 4 - kill (khop=8 >= 1, waited_ms=0)
# parity: KET_LUAN=ok (exit 0)
===== SMP=4 =====
ok 1 - help (khop=16 >= 2, waited_ms=0)
ok 2 - echo (khop=8 >= 1, waited_ms=0)
ok 3 - ps (khop=8 >= 2, waited_ms=0)
ok 4 - kill (khop=8 >= 1, waited_ms=0)
# parity: qemu_con_sot_total=0 qemu_cua_script=0
# parity: KET_LUAN=ok (exit 0)
```

Số hart **thật** trong log boot (không phải hằng trong tài liệu) — khối dưới là dòng của cấu hình
bốn hart; ba hart thứ cấp được `hart_start`:

```text scope=v2/agent/JOURNAL/kt77b-logs/kt77b-run-exit-smp4.log
# smp: boot=0 harts=4 present=0xf online=0xf started=0xe absent=0xf0 timeout=0 dtb=0x87e00000 dtb_magic=1 base=0x80400000 stride=0x1d000
```

## 7. Ba ca âm

**(a) Shell bị tắt** — đúng ảnh sản phẩm canonical **không** feature (sha baseline KT-79). Mọi
mục `active` đỏ ở bước chờ shell, kết luận "không có shell", `exit` năm. Đây là ca âm bắt buộc
của tiêu chí một.

```text scope-red=v2/agent/JOURNAL/kt77b-logs/kt77b-neg-shell-off.log
not ok 1 - help: khong thay shell trong log (qemu_rc=0)
# parity: KET_LUAN=khong_co_shell (exit 5)
```

Log ca âm có dòng đếm QEMU toàn máy khác không ở vài lần chạy **không** phải QEMU của script
này: `qemu_cua_script` bằng không và `verdict` là `OK` ở cả `before` lẫn `after`; QEMU sót thuộc
**tab khác** đang đo song song (các lần chạy ghi ở journal).

**(b) Detector harness** — ảnh sản phẩm **không** có marker harness; ca âm dùng ảnh `ktest-proc`
**canonical, đã strip** (cùng đường remap/strip với ảnh sản phẩm, không phải "bắt nhờ symtab"):

```text scope=v2/agent/JOURNAL/kt77b-logs/kt77b-harness-scan.log
# scan product: hits=0
ok 1 - anh san pham khong chua ma harness (hits=0)
# scan negative: hits=22
ok 2 - ca am: anh co harness bi bat (hits=22 > 0)
# feat product: ktest=0 umode=1
# feat harness: ktest=1
ok 3 - probe gate: cau hinh san pham KHONG bat 'ktest' (probe khong duoc bien dich), co 'umode'
ok 4 - ca am cua probe gate: cau hinh anh test BAT 'ktest' (ktest=1 > 0)
```

Sau khi FS lên ảnh sản phẩm, phép quét được chạy lại trên **ảnh cuối** (KT-77c) — nguồn số cho ảnh cuối là khối dưới:

```text scope=v2/agent/JOURNAL/kt77c-logs/kt77c-harness-scan-final.log
# scan product: hits=0
# scan negative: hits=25
ok 3 - probe gate: cau hinh san pham KHONG bat 'ktest' (probe khong duoc bien dich), co 'umode'
ok 4 - ca am cua probe gate: cau hinh anh test BAT 'ktest' (ktest=1 > 0)
```

**Phạm vi ca âm:** ảnh test `ktest-proc` build **cùng pipeline canonical** (cùng remap + strip) nên
ca âm không "bắt nhờ symtab". Với `probe` (D2-123(1)), literal trần **bị loại** vì PLIC in sẵn
`plic: probe khong khoi phuc priority irq` (dương tính giả); lớp chuỗi dùng symbol **đủ điều kiện**
(`vifast_plat_riscv64::probe`, `probe::on_trap`, `probe_fault_entry`, `src/probe.rs` — chỉ hiện khi
còn symtab), và lớp **feature-graph** (hai dòng `ok` về probe gate) mới là lớp kiểm luôn chạy được trên ảnh
stripped: thiếu `ktest` ⇒ `probe` không được biên dịch.

**(c) Byte vào shell nhưng `expect` sai** — chứng minh kịch bản **không** xanh sẵn: shell in
`# shell: ?` cho dòng sai, runner phân loại "byte đã vào shell" và đỏ (`exit` ba):

```text scope-red=v2/agent/JOURNAL/kt77b-logs/kt77b-neg-wrong-expect.log
not ok 1 - help: byte da vao shell (co 9 dong 'shell: ?') nhung expect khong khop sau 2000 ms (^# shell: echo KHONG-BAO-GIO$)
# parity: KET_LUAN=not_ok (exit 3)
```

## 8. Ngân sách (giữ nguyên ngưỡng)

Ảnh sản phẩm có shell **dưới trần ảnh**; `.data+.bss` dưới trần QD §3.5 (xem khối §2);
`size_of::<Machine>()` **không đổi** vì KT-77b không thêm trường vào core (chỉ dùng lại API sẵn có):

```text scope=v2/agent/JOURNAL/kt77b-logs/kt77b-run-exit-smp1.log
# core: product addr=0x8022a080 bytes=189384 pin=196608 image_cap=262144 untyped=65536 retype=1 configure=1 suspend=1 resume=1 state_ready=1 objects=3 caps=1 used=0 check_caps=1 check_bytes=1 check_all_bytes=1 check_sched=1 footprint_ok=1
```

**Hai chỉ số khác nhau, đừng lẫn:** `.data+.bss` là chỉ số của **cổng ảnh** (QD §3.5, trần ghi ở
khối §2); còn "bộ nhớ tĩnh" của OD-K6-2 là `size_of::<Machine>()` — chỉ số của **core** (`core_bridge`,
không đổi bởi KT-77b), đọc ở khối trên. Hai ngưỡng độc lập; Agent 0 chốt metric.

## 9. Tách phụ thuộc harness (`ktest` → `umode` + `product-shell`)

**Phát hiện** (đúng loại OD-K6-1 cảnh báo, nhưng ở mức **cổng feature** chứ không phải logic):
trước KT-77b, `v2/kernel/vifast-plat-riscv64/Cargo.toml` khai `user = ["ktest"]`, và bốn hàm
**kernel** mà đường U-mode bắt buộc dùng — `copy_kernel_low_root` (`src/obj.rs:201`),
`switch_root` (`src/obj.rs:231`), `restore_kernel_root` (`src/obj.rs:251`), `set_sum`
(`src/obj.rs:262`) — nằm sau `#[cfg(feature = "ktest")]`. Hệ quả: bật U-mode cho ảnh sản phẩm
sẽ kéo luôn `probe` (mã harness, `src/lib.rs:91`) vào ảnh.

**Cách xử lý** (không nhét harness vào ảnh sản phẩm — ngược lại):
- thêm feature **`umode`** ở crate arch = phần **kernel** mà U-mode cần; `user = ["umode"]`;
  bốn hàm trên đổi sang `#[cfg(any(feature = "ktest", feature = "umode"))]`;
- ảnh sản phẩm bật **`product-shell`** (crate kbin) ⇒ `user` ⇒ `umode`, **không** `ktest`;
- kết quả kiểm bằng máy: ảnh sản phẩm **không** có marker harness (§7b).

**Vì sao không ghi `BLOCKED-CHO-A0`**: phụ thuộc tìm được là **gán nhãn feature sai** cho hàm
kernel (không phải U-mode gọi mã test), sửa được **trong** vùng ghi của task mà **không** thêm
một dòng harness nào vào ảnh sản phẩm. Nếu reviewer/Agent 0 đọc khác, bằng chứng nằm ở hiệu ứng
diff: `obj.rs` chỉ đổi **điều kiện cfg**, `Cargo.toml` chỉ đổi **dây feature**, không đổi ngữ
nghĩa ảnh test (kiểm bằng boot lại `ktest-proc` — §10).

## 10. Ảnh test không hồi quy + phần chưa xong

Ảnh `ktest-proc` build lại **sau** thay đổi (tách hằng số sang `proc_abi`, thêm prompt) vẫn
**mười trên mười** TAP, kết thúc đúng `PANIC` chủ đích của ca W^X:

```text scope=v2/agent/JOURNAL/kt77b-logs/kt77b-regress-ktest-proc.log
ok 3 - shell help echo
ok 7 - shell exit va procsrv finalise
PANIC: u-mode fault scause=0xc stval=0x40002000 sepc=0x40002000 sstatus=0x8000000200006000 from_u=1
```

**Chưa xong / nợ (ghi để lead không đọc nhầm là đã đủ OD-K6-2):**

- **Tiêu chí hai mới đạt phần `active`**: `ls`, `cat` (ghi/đọc tệp), `exec`, và đặc biệt
   **đọc lại tệp sau reboot** cần ảnh đĩa persist + FS **trong ảnh sản phẩm** — hiện `vblk`/FS
   chỉ có trong ảnh test, và `spec/v1-parity.toml` đánh các mục đó `deferred-K7`. Kịch bản
   `active` **không** sửa được (spec đóng băng cho KT-77b).
- **Tiêu chí bốn** (ma trận + soak nghìn lần) chưa chạy trong task này; log ở §6 chỉ là
   một/hai/bốn hart cho ảnh sản phẩm — lead sắp bước đo riêng.
- **Ngắt RX thường trực chưa an toàn khi ở U-mode** (trap từ U-mode không đổi `satp`): nợ K7,
   sẽ đóng bằng cách cho trap từ U-mode khôi phục root kernel **trước** khi dispatch ngắt, rồi bỏ
   cửa sổ S-mode.
- **ELF đủ symbol** của ảnh có shell: chờ **KT-84** (D2-122 yêu cầu hai tệp cùng lần build).
- **Ảnh test đổi `.text`** vì `shell.elf` thêm prompt và `proc_demo` đổi nguồn hằng số; ma trận
   và các log `ktest-*` cũ phải chạy lại (TAP kỳ vọng **không** đổi — §10), việc đó thuộc bước đo
   của lead (vùng `v2/test/boot/**` là của KT-78, task này **không** chạm).
- **Repro build & ELF U-mode**: build ở **đường dẫn khác** (git worktree) cho sha khác dù cùng
  commit và cùng `.text` digest — `build.rs` gọi `$RUSTC` trực tiếp cho `init/*_user.rs` nên
  **không** nhận `--remap-path-prefix`; đường dẫn tuyệt đối lọt vào `.rodata`. Nợ cho KT-79/KT-85.
- **Ảnh trên là bằng chứng tại commit `12fbff3e`**: KT-77c đang đổi ảnh (FS/vblk) ⇒ không tuyên
  bố là ảnh phát hành; sau khi commit vá D2-102 phải build lại (sha đổi).
- **`user` không còn kéo `ktest`**: một số mục `pub(crate)` của crate arch (ví dụ
   `user::ranges_save`) chỉ được `exec` dùng, nên khi build `product-shell` trình biên dịch cảnh
   báo "never used". Không phải lỗi; dọn bằng `#[cfg(feature = "exec")]` ở lần chạm tệp sau.

## 11b. KT-137 — `cd`/`pwd` + thư mục hiện tại + đường dẫn tương đối

**Ngày**: 2026-11-18 · **Task**: KT-137 · **Quyết định**: D2-133 (Agent 1 quyết thay Agent 0).

Shell sản phẩm thêm **hai** builtin `cd`/`pwd` (chỉ nhánh `cfg(shell_fs)`), nên `help cmds=` của
ảnh sản phẩm tăng từ mười lăm lên **mười bảy**; ảnh test (không FS) **vẫn sáu** nên
`proc_demo.rs`/`ktest.sh` không phải sửa. Shell giữ thư mục hiện tại trong buffer cố định và
**tự chuẩn hoá từ vựng** (`.`/`..`/`//`) trước khi gửi `OP_PATH` — `vifast-fs` vốn từ chối hai
tên `.`/`..` và đòi đường tuyệt đối, nên **không** đổi ngữ nghĩa FS, **không** đổi giao thức FS
server, **không** mở ABI. Mọi lệnh có tham số đường dẫn (`ls`/`cat`/`write`/`truncate`/`stat`)
nhận **đường tương đối** so với thư mục hiện tại; `ls` không tham số liệt kê thư mục hiện tại.

Hành vi đã ghim: `cd` không tham số về `/`; `cd ..` ở gốc **ở lại** `/`; `cd` vào đường không có
hoặc vào **tệp** báo lỗi và **không** đổi thư mục hiện tại; `pwd` in `# shell: pwd <đường>`.

Bằng chứng: ca `kt137-cdpwd` của `v2/test/boot/shell-edge.sh` (mười một phép kiểm, mỗi lệnh một
dòng phán quyết) + nhật ký `v2/agent/JOURNAL/kt137-cdpwd.md` (ba vế: đỏ trên mã cũ, xanh trên mã
mới, đột biến).

## 11. Tái lập (lệnh có tham số dạng chữ, để tài liệu không chứa số đo viết tay)

```bash
# ảnh sản phẩm CÓ shell (canonical, stripped) — in dòng REPRO sha256/size/build/text
VIFAST_REPRO_CARGO_EXTRA="--features product-shell" bash v2/tools/repro-build.sh
# ca âm shell tắt (ảnh đúng baseline KT-79)
bash v2/tools/repro-build.sh
# copy ảnh đo vào artifacts/<sha256>.bin (chỉ đọc, không ghi) rồi boot từ tệp đó
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/<sha256>.bin --smp N
# ca âm shell tắt / sai expect
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/<sha-cua-anh-khong-shell>.bin --smp N
bash v2/test/parity/run.sh --mode serial --image v2/artifacts/<sha256>.bin --smp N --negative wrong-expect
# detector harness (bắt buộc kèm ca âm)
bash v2/agent/JOURNAL/kt77b-harness-scan.sh \
  --image v2/artifacts/<sha256>.bin --negative v2/artifacts/<sha-anh-test>.bin
```
