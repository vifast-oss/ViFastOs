# KT-316 — khoá COM1 (một dòng log không bị hart khác xen ký tự)

Nhánh `task/kt316-comlock`, base `main` @ `e0d58d5e`. Người làm: sub-agent `comlock` (lead uỷ
quyền commit theo D2-198).

## 1. Khảo sát trước (bắt buộc — trả lời ba câu hỏi của phiếu)

### 1.1 `serial.rs` ghi từng byte hay từng dòng?

**Không ghi gì cả.** `v2/kernel/vifast-plat-x86_64/src/serial.rs` là **dữ liệu thuần** (địa chỉ
thanh ghi, bit `LSR`, divisor, thứ tự khởi tạo) — không có hàm ghi. Đường ghi thật nằm ở ảnh:

- `v2/kernel/vifast-kbin-x86_64/src/main.rs:497` — `impl Write for Console`:
  `write_str` lặp **từng byte** gọi `write_byte`.
- `main.rs:519` — `write_byte`: chờ `LSR.THRE` (trần `TX_WAIT_LIMIT`) rồi `outb(0x3F8, byte)`.

⇒ Đường ghi là **từng byte**, **không có khái niệm "dòng"** ở tầng nào. Vì vậy khoá phải đặt ở
tầng **một lời gọi định dạng** (`write_fmt`), không phải tầng byte.

### 1.2 `Console::write_str` được gọi bao nhiêu lần cho một `writeln!`?

**Nhiều lần — không phải một.** `writeln!` mở ra `write_fmt(format_args!(…))`; impl mặc định của
`core::fmt::Write::write_fmt` gọi `write_str` cho **từng mảnh literal và từng tham số**. Đo bằng
chương trình host (cùng `core::fmt`, không phải suy đoán):

```text
writeln! -> 3 write_str calls: ["# sched: ticks=", "10", " tasks=3\n"]
3 write! -> 4 write_str calls: ["[", "0x", "2a", "]"]
```

Hệ quả: khoá **quanh `write_str`** vẫn để hai hart xen ký tự ở **ranh giới mảnh** (hart A in
`# sched: ticks=`, hart B in `# smp: hart=1 online`, rồi A in `10 tasks=3`). Đó đúng là lớp lỗi
soak vòng 21 bắt được (`# exc:`/`PANIC` bị cắt ghép). Vì vậy KT-316 khoá **toàn bộ `write_fmt`**
(một `writeln!`/`write!` = một dòng) và giữ `write_str` có khoá cho đường gọi trực tiếp.

### 1.3 Có chỗ nào ghi trong **ngắt** (⇒ khoá phải `cli`)?

**Có, ba đường in trong ngắt** (đều chạy với `RFLAGS.IF = 0` vì gate kiểu interrupt):

| Đường | Vector | In gì |
|---|---|---|
| `main.rs::exc_dispatch` | mọi exception | `# exc: vec=…` + `# exc: frame …` |
| `smp.rs::note_ipi` | `0x30` (IPI) | `# smp: ipi recv=<n> intid=0x30` |
| `sched.rs::mh_tick` (hart phụ) | 32 (timer) | `# sched: hart=<h> picked=…` |

Ngoài ra `umode.rs::print_user` in **từng byte** trong handler `int 0x80` (syscall `DebugPutStr`)
— cũng là ngắt, cũng phải khoá.

⇒ Nếu chỉ CAS mà **không** `cli`: hart đang giữ khoá bị ngắt **trên chính nó**, handler in ⇒
handler quay lại `lock()` trên khoá **của chính hart ấy** ⇒ **tự deadlock**. Vì vậy `cli` **trước**
khi lấy khoá và khôi phục `IF` **sau** khi nhả. Khôi phục theo **giá trị đã lưu** (không `sti`
mù): handler đã có `IF = 0` thì nhả khoá **không** được bật ngắt giữa handler.

### 1.4 Vì sao KT-298 **cố ý** không khoá (nay lý do hết hiệu lực)

`v2/agent/JOURNAL/kt298-x86_64-gp.md` §8.2: "hai hart cùng in vẫn xen ký tự. Đó là **triệu
chứng** từng thấy, không phải gốc; thêm khoá sẽ che mất lớp lỗi này thay vì sửa nó." Lý do ấy
đúng khi **chưa** có đường hai hart cùng `schedule()` thật: dòng xen ký tự là **manh mối** duy
nhất của "hai hart chung một stack". Nay KT-310/KT-315 đã có **nhiều hart schedule thật** (2 → 4
hart), nên dòng `# exc:`/`PANIC` bị cắt ghép **không còn là manh mối** mà là **lỗi chẩn đoán**:
người đọc log đọc sai nguyên nhân. Comment `sched.rs:795-798` khẳng định "không có hai dòng xen
ký tự" là **sai sự thật** — KT-316 sửa comment và thêm khoá.

## 2. Hiện thực

(xem §3..§6 — điền sau khi làm)

## 3. Hiện thực

### 3.1 `vifast-plat-x86_64::com` — khoá CAS + `cli`

Tệp mới `v2/kernel/vifast-plat-x86_64/src/com.rs`:

- `ComLock` — `AtomicBool` CAS (`Acquire`/`Release`) quay vòng; bộ đếm `acquired`, `contended`,
  `overlap`, `in_cs`.
- `lock()` → `enter_critical()` (`asm::interrupts_enabled()` + `asm::cli()`) → CAS → guard.
- `ComGuard::drop` → `in_cs -= 1` → `locked = false` (`Release`) → `leave_critical(was_enabled)`
  (chỉ `sti` nếu trước đó `IF = 1`).
- `ok()` = `acquired > 0 && overlap == 0` — "đã dùng khoá **và** chưa từng chồng".
- `pub static COM1: ComLock` — một khoá duy nhất cho cả ảnh.

`asm.rs` thêm `interrupts_enabled()` (`pushfq; pop` — cặp cân bằng stack, không đổi cờ) + stub
host. Trên host, `enter_critical`/`leave_critical` là no-op ⇒ test `cargo test` đo được phần **loại
trừ** (CAS) bằng nhiều luồng thật.

### 3.2 Ảnh: khoá ở `write_fmt`, không phải `write_str`

`v2/kernel/vifast-kbin-x86_64/src/com.rs` (tách khỏi `main.rs` vì R1 — `main.rs` 1.499 → 1.489
dòng):

- `Console::write_fmt` → `with_lock(|raw| fmt::write(raw, args))` — **cả một dòng** nguyên tử.
- `Console::write_str` → `with_lock(...)` — đường gọi trực tiếp.
- `RawConsole` — đường ghi **không khoá**, chỉ dùng khi đã giữ khoá.
- `com_guard()` — `None` khi `--features no-com-lock` (đột biến i).
- `com_report()` — in `# com: lock ok=<0|1> contended=<n>` + TAP 28.

Ba đường còn lại cũng được bọc:

| Đường | Sửa |
|---|---|
| `umode::print_user` (syscall `DebugPutStr`, in từng byte trong `int 0x80`) | giữ khoá cho **cả chuỗi** |
| `smp::write_hex_list` (một dòng ghép từ 5 `write!`) | `console.with_lock(...)` cho **cả dòng** |
| `panic_handler` | dùng `Console` ⇒ tự có khoá |

### 3.3 Sửa comment sai

- `v2/kernel/vifast-kbin-x86_64/src/sched.rs:795-798` (comment KT-310) — nói đúng: **trước
  KT-316 hai hart có xen ký tự** (soak vòng 21), KT-298 cố ý không khoá vì khi đó đó là manh mối;
  nay nhiều hart `schedule()` là tính năng nên xen ký tự chỉ làm sai chẩn đoán; khoá bảo đảm dòng
  nguyên vẹn.
- Hai comment cùng lớp: `sched.rs` (`mh_tick`, "in trước, đếm sau") và `smp.rs::note_ipi` — "in
  trước" chỉ bảo đảm **thứ tự**, không bảo đảm nguyên vẹn; nguyên vẹn là nhờ khoá.

## 4. Bằng chứng

### 4.1 Test đỏ-trước (D2-05)

`v2/kernel/vifast-plat-x86_64/tests/com.rs` viết **trước** khi có mô-đun:

```text
error[E0432]: unresolved import `vifast_plat_x86_64::com`
  --> vifast-plat-x86_64/tests/com.rs:24:25
   |
24 | use vifast_plat_x86_64::com::ComLock;
   |                         ^^^ could not find `com` in `vifast_plat_x86_64`
```

Sau khi hiện thực: `3 passed` (bốn luồng × 100 lần đọc–sửa–ghi trong vùng găng ⇒ đúng 400;
`overlap = 0`; `contended ≥ 1` khi luồng chính giữ khoá).

### 4.2 QEMU — dòng `# com:` nguyên văn

```text
# com: lock ok=1 contended=0
ok 28 - com: lock
```

(59 boot `--smoke 60 3` đều `ok=1`, `contended=0` — boot bình thường **không** có hai hart in
đồng thời, đúng thiết kế "in trước, đếm sau"; khoá là lưới an toàn cho đường lỗi.)

### 4.3 Runner

`bash v2/test/boot/run-x86_64.sh`:

```text
ok 2 - boot 1: TAP 28/28 + 5 dòng '# exc:' + marker 1 lần + halt sạch
...
ok 23 - ca âm (u): dòng '# com:' bị xen ký tự ⇒ ĐỎ (# dòng '# com: lock ok=1 contended=<n>' xuất hiện 0 lần, cần đúng 1)
# ca âm: 21/21 ĐỎ đúng như phải
# 20/20 kiểm sạch (boot=1, tests=28, exc=5)
```

## 5. Đột biến

### 5.1 (i) `--features no-com-lock` — bỏ khoá

10/10 boot: `# com: lock ok=0 contended=0` + `not ok 28 - com: lock` + `# TAP 28 FAIL — dừng`
(ĐỎ **tất định**; QEMU không tự thoát ⇒ runner timeout 124, đúng đường "không marker").

Log **không** xen ký tự trong 10 boot bình thường — **đúng như dự đoán**: boot bình thường in
tuần tự theo thiết kế ("in trước, đếm sau"), nên triệu chứng gốc chỉ xuất hiện khi hai hart in
**đồng thời** (đường lỗi). Để chứng minh khoá thật sự chống xen ký tự, tôi dùng **probe tạm**
(không commit): BSP và hart 1 hẹn giờ bằng cờ rồi cùng in một dòng 64 ký tự.

| Ảnh | Dòng `# com: race hart=1 …` | Kết quả |
|---|---|---|
| có khoá | `# com: race hart=1 BBBB…(64) end` | **5/5 nguyên vẹn** |
| `no-com-lock` | `# com: race hart=#1 BBBBBBBBBBB com: race hart=BBBBBB0 AAAA…BBBB… en end` | **5/5 xen ký tự** |

Dòng xen ký tự nguyên văn (log `/tmp/kt316-race-nolock/boot-1.log`):

```text
# com: race hart=#1 BBBBBBBBBBB com: race hart=BBBBBB0 AAAABBBBBBBBBBBBBBBBBBBBBBAAAAAAAAAAAAABBAAAAAAAAAABBBBAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBBBBBBBBBBBBBBBBBAAAAB en end
```

### 5.2 (ii) `--features no-com-cli` — khoá mà không `cli`

Probe tạm: hart 1 giữ khoá COM1 rồi **dừng giữa dòng**; BSP gửi IPI (handler IPI **in**) rồi mới
cho hart 1 in nốt.

| Ảnh | Kết quả 3 boot |
|---|---|
| có `cli` | dòng hart 1 **nguyên vẹn**, `# smp: ipi recv=2` (IPI bị **hoãn** tới sau khi nhả khoá), marker + halt sạch — 3/3 |
| `no-com-cli` | dòng hart 1 **cụt** (`# com: race hart=1 ` rồi treo), **không** `ipi recv=2`, **không** marker, QEMU treo (rc=124) — 3/3 |

Đây đúng là **tự deadlock**: handler IPI chạy trên chính hart 1 khi nó đang giữ khoá ⇒
`COM1.lock()` quay vòng trên khoá của chính nó. `cli` là thứ ngăn cửa sổ ấy.

## 6. Còn gì chưa xong / rủi ro

1. **`contended=0` ở mọi boot bình thường** — không phải lỗi: ảnh in tuần tự theo thiết kế. Khoá
   chỉ có việc khi hai hart in đồng thời (đường lỗi). Đã chứng minh bằng probe tạm (§5.1).
2. **Probe in đua không commit** — nếu muốn đo `contended > 0` thường xuyên, cần một pha in đua
   **có chủ đích** trong ảnh; việc đó đổi nội dung log nên **không** làm trong KT-316.
3. **`soak --smoke 60 3` lần đầu 59/60** — boot 41 đỏ TAP 25 `sched: multi-hart` vì
   `first=[a, a]` (không phải `# com:`). Đây là **giả định quá mạnh của KT-310**: `first[hart]` là
   tác vụ hart ấy chọn ở **tick đầu của nó**, hai hart có thể chọn cùng một tên nếu tác vụ đã
   được trả về hàng đợi giữa hai tick (`double=0` chứng minh **không** double-pick). Xem §7.
4. **Không** tuyên bố K5' đạt.

## 7. `soak --smoke 60 3` — kết quả và flake TAP 25 (không thuộc KT-316)

| Lượt | Kết quả | Ghi chú |
|---|---|---|
| 1 (ảnh KT-316) | **59/60** | boot 41: `not ok 25 - sched: multi-hart`, `first=[a, a] double=0` |
| 2 (phân loại, D2-07) | **60/60** | 60/60 `# com: lock ok=1 contended=0` + `ok 28` |
| nền (`e0d58d5e`, worktree sạch) | **60/60** | không tái hiện `first=[a, a]` trong 60 boot |

**Phân tích `first=[a, a]`** (log boot 41):

```text
# sched: hart=1 picked=a n=1
# sched: hart=0 picked=a n=4
# sched: multi-hart harts=2 rounds=6 picks=[2, 4] first=[a, a] double=0
not ok 25 - sched: multi-hart
```

`first[hart]` là tên tác vụ hart ấy chọn ở **tick đầu của nó**, không phải "hai hart chọn cùng
lúc". Hart 1 chọn `a` ở vòng 1, chạy rồi **trả `a` về hàng đợi**; hart 0 tới tick đầu ở vòng 4 và
chọn `a`. `double=0` chứng minh **không** có hai hart cùng sở hữu một tác vụ. Vì vậy
`first[0] != first[1]` **không phải bất biến** — nó là quan sát xác suất của KT-310 (KT-310 chỉ
đo một boot `first=[b, a]`). Đây là **giả định sai của phép kiểm KT-310**, không phải lỗi của khoá
COM1: 119 boot có `# com:` đều `ok=1`, và boot 41 đỏ **trước** khi tới TAP 28.

**Không sửa trong KT-316**: sửa `mh_ok`/runner/soak của KT-310 là thay đổi hợp đồng của task
khác (và dễ bị đọc là "nới ngưỡng"). Đề xuất task riêng: bỏ `first[0] != first[1]`, thay bằng bất
biến **thật** — `double == 0` ∧ `picks[1] >= 1` ∧ mọi hart đã chọn (đã có) — hoặc ghi `first` kèm
**vòng** của từng hart để phân biệt "cùng lúc" với "cùng tên khác vòng".

## 8. Xác nhận

- **Không** push, **không** merge, **không** chạm `main`.
- **Không** sửa `v2/agent/{STATE,QUEUE,DECISIONS}.md`, **không** sửa `v2/bin/v2`, **không** sửa
  `spec/`.
- **Không** nới ngưỡng, **không** `#[ignore]`/retry, **không** `cfg(target_arch)` trong
  `vifast-core`/`vifast-plat-common`.
- Commit bằng `VIFAST_ROLE=lead git commit` (uỷ quyền D2-198); `cargo fmt` đã chạy trước commit.
- `./v2/bin/v2 check` **PASS**; `bash v2/test/boot/run-x86_64.sh` **xanh** (TAP 28/28, 21/21 ca
  âm); `soak --smoke 60 3` **60/60** ở lượt phân loại (lượt đầu 59/60 do flake KT-310, §7).
