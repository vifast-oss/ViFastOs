# BLOCKED-CHO-A0 — việc Agent 1 KHÔNG được tự quyết (D2-121)

Mỗi mục: câu hỏi cụ thể + bằng chứng + đề xuất. Agent 1 **không** tự chốt các mục này.

---

## B1 · Ảnh nào là **ảnh phát hành** của v2.0? — **ĐÓNG 2026-11-17 (D2-122.2): chọn (B) canonical**

**ĐÃ GIẢI**: Agent 0 chọn **(B)** — ảnh sản phẩm = đầu ra pipeline canonical, áp dụng **ngay**; xuất 2 tệp cùng lần build (ảnh phát hành strip + ELF đủ symbol), ghi sha cả hai; `fd202276` giữ log nhưng **không** dùng làm bằng chứng v2.0; **không** chạy lại soak/matrix/persist trên `c1072d72` ngay — chạy **một lần** trên ảnh canonical **có shell** sau KT-77b; ngưỡng ≤ 262.144 đo trên ảnh canonical. Cấu trúc chống tái diễn: **KT-84** (D2-122.6).

**Bối cảnh cũ (giữ để tra cứu)**: OD-K6-3 đã đạt trên **bản canonical** (`sha256=c1072d72…`, `size=82552`, `-C strip=symbols` + remap + metadata chuẩn hoá) ở **2 máy × 2 đường dẫn**. Nhưng **ảnh phát hành hiện tại** là bản **trơn** `fd202276…/188552` (giữ symbol) — bản này **chưa** chứng minh tái lập.

Hai đường, chỉ Agent 0 chọn được:

| | (A) Giữ bản trơn `fd202276` | (B) Chuyển ảnh phát hành sang canonical `c1072d72` |
|---|---|---|
| Tái lập byte | **chưa** chứng minh | **đã** chứng minh (2 máy × 2 đường dẫn) |
| Kích thước | 188.552 | **82.552** |
| Bằng chứng đang có | soak `0/1000`, matrix `21/21`, persist `7/7` trên **chính sha này** | **phải chạy lại toàn bộ** trên sha mới |
| `.text` digest | `build=ef566cfa583e34bb text=49874` | `build=2cbe9bbe339d7c02 text=49874` |
| Rủi ro | “tương đương v1” với một artifact **không** tái lập được | phải soak lại; strip làm mất symbol (khó debug) |

**Đề xuất của Agent 1 (không tự quyết)**: chọn **(A) cho v2.0**, và ghi trong văn bản đóng K6 rằng tái lập byte đạt trên **pipeline canonical** (`v2/tools/repro-build.sh`) chứ **chưa** cho ảnh phát hành; nợ P20 chuyển sang v2.1 với tiêu chí “ảnh phát hành = ảnh canonical”. Lý do: (B) buộc chạy lại soak 1.000 + matrix 21 + persist 10 boot, và strip làm mất khả năng đọc symbol khi có lỗi.

**Bằng chứng**: `v2/agent/JOURNAL/kt79-repro-check.log`, `kt79b-ps-builds.log`, `kt79b-macos-canonical.log`, `kt79-negative.log`; `v2/docs/k6-repro.md` §5.1, §6.

---

## B2 · Windows — **ĐÓNG 2026-11-17 (D2-122.4): thành nợ v2.1**

Host **Windows** (`psvn`) **không** hội tụ sha: khác **thứ tự monomorphization** (`core::fmt::Display::fmt` do crate khác sở hữu). Nguyên nhân là lựa chọn của rustc/LTO theo host, **không** phải mã nguồn. Muốn hội tụ cần `-Z build-std` (nightly) hoặc chuẩn hoá tầng linker.

**Câu hỏi**: v2.0 có cần tái lập trên **Windows** không? Nếu không, ghi “v2.0 chỉ tái lập trên Linux + macOS” là đủ.
**Đề xuất**: **không** cấp ngân sách; ghi giới hạn host Windows vào `k6-repro.md` §7 và nợ v2.1.

---

## B3 · K6 ĐẠT/không ĐẠT — chờ bằng chứng KT-77b (mở 2026-11-17)

Agent 1 **không** được tuyên bố mốc. Khi KT-77b xong hoặc hết ngân sách (OD-K6-1), Agent 1 sẽ trình: log serial trên ảnh sản phẩm, ca âm tắt shell, bằng chứng máy “không chứa harness”, kích thước ảnh, danh sách `deferred-K7`. **Quyết định đạt/không đạt thuộc Agent 0.**

---

## B4 · `cd` / `pwd` có thuộc “tương đương v1” không? — **ĐÓNG 2026-11-21 (D2-197): ĐƯA VÀO** (mở 2026-11-17, từ review KT-77a)

**ĐÓNG 2026-11-21 (D2-197)**: Agent 0 (uỷ quyền Agent 1) chốt **đưa vào** danh sách “tương đương
v1”: bản **đóng băng** `v1-archive` có `cd`/`pwd` (`crates/vifast-client/src/main.rs:878-912`, help
`:917`) ⇒ “tương đương v1” so với **bản đóng băng**, không so với ý định đã revert
(`docs/v1/overnight-log.md:3664-3685`). Hai mục `cd`/`pwd` trong `spec/v1-parity.toml` (dòng
120-146) **đã** `status = 'active'` từ KT-137, giữ nguyên văn bằng chứng hai chiều
(`counter_evidence` + `decision` + `reason`).

**Bằng chứng đo (KT-243, 2026-11-21)**: runner parity chạy **thật** trên ảnh sản phẩm canonical
`product-fs` (`sha256=5bd28bec…`, `size=213632`, `inputs=54f3b59fc08f3221`, `dirty=0`), chế độ
`--mode serial` (byte bơm qua stdin QEMU), **14/14 mục `active` xanh, exit 0**:
`v2/agent/JOURNAL/kt243-b4-parity.log:622` `ok 5 - cd (khop=8 >= 1, waited_ms=0)`;
`:732` `ok 6 - pwd (khop=8 >= 1, waited_ms=0)`; `:1657` `# parity: KET_LUAN=ok (exit 0)`.
Đầu ra shell thật: `:604` `# shell: cd /`, `:714` `# shell: pwd /`. Chi tiết:
`v2/agent/JOURNAL/kt243-b4b6b7.md` §2.

**Câu hỏi**: mã v1 trong tag `v1-archive` **có** `cd`/`pwd` (`crates/vifast-client/src/main.rs:878-912`), có trong **help của v1** (`:917`) và trong bảng sinh tự động của v1 (`docs/11-hien-tai/16-van-hanh.md:109,121`). Nhưng `docs/v1/overnight-log.md:3664-3685` ghi vòng **222/223 đã revert** `cd`/`pwd`; tag `r323` lại có.

**Tại sao cần A0**: đưa vào hay loại bỏ đều **đổi định nghĩa “tương đương v1”** — Agent 1 không được quyết (D2-121: không đổi kỳ vọng, không hạ/không mở phạm vi cổng).

**Trạng thái hiện tại (đã ghi, không im lặng)**: KT-77a đưa 2 mục vào `spec/v1-parity.toml` với `status = 'deferred-K7'`, `decision = "cho Agent 0"`, và **cả hai chiều bằng chứng** (v1 code có / overnight-log ghi revert).

**Đề xuất của Agent 1 (không tự quyết)**:
- Nếu v1 lúc **đóng băng** (`v1-archive`) thật sự có `cd`/`pwd` chạy được ⇒ **đưa vào** danh sách (vì “tương đương v1” phải so với **bản đóng băng**, không phải so với ý định đã revert).
- Nếu overnight-log cho thấy chúng bị revert **trước** khi đóng băng ⇒ **loại**, và ghi một dòng vào `v1-parity.toml` rằng đã xét và loại, kèm `file:dòng`.
Cần A0 chọn; KT-77b **không** phụ thuộc mục này (shell trên ảnh sản phẩm vẫn là tiêu chí 1-3).

---

## B5 · ~~Cổng `# build: rev` quá chặt~~ **ĐÓNG 2026-11-17 bởi D2-127** — cổng sẽ so nội dung build-inputs, không so lịch sử git. Ghi giữ làm bằng chứng. khi hai dòng cùng commit vào `main` (mở 2026-11-17, bằng chứng KT-88)

**Hiện tượng (có log)**: KT-88 build ảnh `38714b79…` ở HEAD `d40f1e63a61d`; boot smp=1 **ACCEPT**; 4 phút sau dòng ViScript commit 3 lần (`7adcbf6e`, `4e903d94`, `ffe75569`) vào **cùng `main`** ⇒ smp=2 **`REFUSE reason=rev-mismatch`**, smp=4 cũng vậy. Toàn bộ ảnh bị từ chối.

**Nhưng mã kernel KHÔNG đổi**: `git diff d40f1e63..HEAD -- v2/kernel v2/lib` = **rỗng**; `git status -- v2/kernel v2/lib` sạch (`dirty=0`). Ba commit kia chỉ chạm `viscript/`, `docs/`, `v2/agent/`.

**Vấn đề**: cổng so `rev` trong ảnh với **12 hex đầu HEAD hiện tại**. Ý định của D2-125.3 là "ảnh phải khớp **mã kernel đã commit**", không phải "không ai được commit bất cứ đâu". Với hai dòng sản phẩm dùng chung `main`, điều kiện hiện tại làm **mọi phép đo dài bị vô hiệu** bởi commit không liên quan — và tệ hơn: nó buộc phép đo cuối K6 phải **đóng băng `main` cho cả hai dòng**, điều không bền vững.

**Đề nghị của Agent 1 (need A0 — đổi phương pháp đo thuộc quyền A0, D2-121)**:
Chấp nhận khi **cả hai** đúng:
1. `image_rev` **là tổ tiên** của HEAD (`git merge-base --is-ancestor <image_rev> HEAD`), và
2. `git diff <image_rev>..HEAD -- v2/kernel v2/lib` **rỗng**.

Như vậy: ảnh cũ + mã kernel đã đổi ⇒ **vẫn bị từ chối** (đúng ý đồ); ảnh cũ + chỉ dòng khác commit ⇒ **được đo** (đúng ý định). Không nới ngưỡng nào, không bỏ kiểm `dirty`, không bỏ kiểm `image-mismatch`.

**Không tự sửa cổng**: Agent 1 giữ nguyên `build-rev.sh`; nếu A0 duyệt thì giao task riêng (kèm **ca hồi quy**: (i) sửa 1 dòng kernel ở HEAD sau ⇒ phải TỪ CHỐI; (ii) chỉ commit `docs/` ⇒ phải ACCEPT; (iii) ảnh không phải tổ tiên ⇒ TỪ CHỐI) và **cả hai dòng** phải ghi vào luật.

**Tình trạng hiện tại**: người dùng đã đồng ý **đóng băng `main` cả hai dòng ~60 phút** để KT-88 đo đúng một lần ở HEAD `ffe75569e0b2` (D2-123.3). Biện pháp tạm, không phải giải pháp.

---

## B6 · build-inputs của D2-127 bao gồm `v2/agent/JOURNAL/**` ⇒ mọi phép đo tự vô hiệu — **ĐÓNG 2026-11-21 (D2-197)** (mở 2026-11-17, TRƯỚC KT-89)

**ĐÓNG 2026-11-21 (D2-197)**: Agent 0 (uỷ quyền Agent 1) chốt định nghĩa **D2-127 mục (3) sửa
đổi** = (bao đóng phụ thuộc theo `cargo metadata` của kernel/plat/lib/kbin) ∪ (tệp trong `v2/` mà
ảnh phụ thuộc: `Cargo.lock`, `rust-toolchain.toml`, linker script, script build, `v2/kernel/**`,
`v2/lib/**`) **TRỪ** `v2/agent/**`, `v2/docs/**`, `v2/target*/**`, `v2/artifacts/**`.
`v2/test/boot/build-rev.sh` đã cài đúng (dòng 13-19, 141-148, 196).

**Bằng chứng đo (KT-243, 2026-11-21)**:
1. `bash v2/test/boot/build-rev.sh --selftest` ⇒ **PASS (a/b/c/d/e/f/g/h/i/j/k)** (11 ca).
   **Ca dương còn thiếu đã thêm** — nhưng đặt ở `v2/test/boot/build-identity.sh --selftest` ca
   **(l6)**, **không** ở `build-rev.sh`: `build-rev.sh` bị **ghim** bởi `check-algo-pin.sh` (sửa tệp
   ⇒ buộc tăng `VBREV_ALGO` ⇒ mọi ảnh cũ thành "định nghĩa khác") trong khi thuật toán băm **không
   đổi một byte**; `build-identity.sh` là tệp **tiêu thụ** định nghĩa, đúng lý do nó được tách ra ở
   KT-145. Ca (l6): tạo tệp **chưa commit** dưới `v2/agent` + `v2/docs` (đúng cách harness ghi log
   trong lúc đo) ⇒ dấu vân tay **không đổi**, tệp **không** vào danh sách bẩn, quyết định của cổng
   **không đổi**. `bash v2/test/boot/build-identity.sh --selftest` ⇒ **PASS (l1/l2/l3/l4/l5/l6)**.
   Đột biến thêm `v2/agent` vào `VBREV_INPUT_ROOTS` ⇒ ca (l6) **ĐỎ**, hoàn nguyên ⇒ xanh — log
   `v2/agent/JOURNAL/kt243-b6-case-l6-dotbien.log`.
2. **Ca dương bắt buộc ở mức sha ảnh**: ba lần build ảnh canonical `product-fs` — (1) cây sạch,
   (2) chỉ sửa `v2/docs`+`v2/agent` chưa commit cùng target dir, (3) build **lạnh** ở target dir
   mới với thay đổi docs/agent còn nguyên — **cùng** `sha256=5bd28bec…`, `size=213632`,
   `inputs=54f3b59fc08f3221`, `dirty=0` ⇒ phần TRỪ **không** ảnh hưởng byte ảnh. Log
   `v2/agent/JOURNAL/kt243-b6-build.log`; chi tiết `v2/agent/JOURNAL/kt243-b4b6b7.md` §3.
   *Nợ ghi rõ*: ca sha-ảnh đo trực tiếp, **chưa** tự động hoá thành bước cổng (2-3 lần build ảnh
   ≈ 30-45 s); ca hồi quy thường trực là (a) của `build-rev.sh` + (l6) của `build-identity.sh` ở mức dấu vân tay + dirty + cổng.

**Câu hỏi**: mục (3) của D2-127 định nghĩa build-inputs = “toàn bộ `v2/`”. Nhưng harness **ghi log trong lúc đo**, log nằm ở `v2/agent/JOURNAL/…`.

**Hệ quả nếu giữ nguyên**: dòng log đầu tiên làm dấu vân tay đổi ⇒ lần đo kế tiếp **REFUSE** ⇒ **mọi phép đo tự vô hiệu chính nó**. Đây không phải giả thuyết: KT-86 đã phải định nghĩa `dirty` chỉ trong `v2/kernel` + `v2/lib` **vì đúng lý do này**, và đã kiểm bằng thực nghiệm rằng ghi log **không** làm `dirty=1`.

**Vênh thứ hai**: mục (3) nói danh sách “do script sinh từ `cargo metadata`”, nhưng `v2/agent/**` và `v2/docs/**` **không** nằm trong `cargo metadata` của bất kỳ crate nào.

**Đề nghị Agent 1** (không tự quyết — đổi định nghĩa cổng là quyền A0):
```text
build-inputs = (bao đóng phụ thuộc theo cargo metadata của kernel/plat/lib/kbin)
             ∪ (tệp trong v2/ mà ảnh phụ thuộc: Cargo.lock, rust-toolchain.toml,
                linker script, script build, v2/kernel/**, v2/lib/**)
             TRỪ: v2/agent/**, v2/docs/**, v2/target*/**, v2/artifacts/**
```
kèm **ca hồi quy dương bắt buộc**: chỉ sửa `v2/docs` hoặc `v2/agent` ⇒ build 2 lần **cùng sha** (chứng minh phần trừ không ảnh hưởng byte ảnh), bổ sung cho 5 ca âm của mục (6).

**Trạng thái**: KT-89 **chưa** bắt đầu; cần A0 chốt trước, nếu không KT-89 sẽ “xong” theo đúng chữ nhưng cổng tự khoá chính nó.

---

## B7 · Test `smp` ĐỎ SẴN và **không cổng nào chạy** — **ĐÓNG 2026-11-21 (D2-197): chọn (A)** (mở 2026-11-18)

**ĐÓNG 2026-11-21 (D2-197)**: Agent 0 (uỷ quyền Agent 1) chọn **(A)**: sửa **test + chú thích** cho
khớp `HART_STRIDE` = `GUARD + STACK + TRAP + SYSCALL`, **và** thêm bước **L0 workspace kernel** vào
cổng. **Đã thi hành ở KT-148** (`9163630d`, D2-128); KT-243 **xác minh lại bằng đo**, không sửa mã:

1. `v2/kernel/vifast-plat-common/tests/smp.rs:24-27` — công thức **bốn** số hạng; chú thích đầu tệp
   `v2/kernel/vifast-plat-common/src/smp.rs:9-11` cũng đủ bốn số hạng.
2. **Bước gate phủ nó**: `v2/bin/v2:892` `lane_step main 42 "L0 kernel workspace (KT-148)"
   step_l0_kernel` → `v2/bin/v2:603-606` chạy `cargo test --manifest-path v2/kernel/Cargo.toml
   -p vifast-plat-common -p vifast-plat-aarch64 -p vifast-plat-x86_64`; bước **không** nằm trong
   `V2_T2_ONLY_STEPS` (`v2/bin/v2:23`) ⇒ chạy ở **cả T1 (`gate --dev`) và T2 (`gate --fast`)**.
   `./v2/bin/v2 check` **không** chạy test (check = fmt/clippy/luật + build) — ghi rõ, không thêm
   bước gate trùng.
3. **Đo xanh**: `cargo test … -p vifast-plat-common --test smp` ⇒ **9/9 PASS**.
4. **Ca âm bắt buộc** (`v2/agent/JOURNAL/kt243-b7-red.log`): tạm hoàn nguyên công thức ba số hạng ⇒
   đúng `left: 86016 right: 118784`, `FAILED. 8 passed; 1 failed`, RC=101; chạy **đúng lệnh của bước
   gate** với đột biến ⇒ **RC=101 (bước gate ĐỎ)**; hoàn nguyên ⇒ RC=0, 18 passed.
   Chi tiết: `v2/agent/JOURNAL/kt243-b4b6b7.md` §4.

**Phát hiện** (khi chạy L0 của workspace kernel trong KT-81 §A5b):

```text
$ cargo test --manifest-path v2/kernel/Cargo.toml -p vifast-plat-common --test smp
---- bo_cuc_per_hart_khong_chong_va_can_trang stdout ----
thread '…' panicked at vifast-plat-common/tests/smp.rs:23:5:
assertion `left == right` failed
  left: 86016
 right: 118784
test result: FAILED. 8 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

- `86016` = `HART_GUARD_BYTES + HART_STACK_BYTES + HART_TRAP_STACK_BYTES` (4K + 64K + 16K).
- `118784` = `HART_STRIDE`, **đã** gồm `HART_SYSCALL_STACK_BYTES` (32K, thêm ở KT-66).
- Chú thích đầu `v2/kernel/vifast-plat-common/src/smp.rs:10-11` cũng ghi công thức **thiếu** số hạng
  syscall stack ⇒ **hai chỗ cùng cũ**, không phải test sai một mình.
- `smp.rs` (nguồn **và** test) **không** bị sửa trong task này ⇒ **đỏ sẵn từ trước**.

**Vì sao không ai thấy**: `v2/Cargo.toml` có `exclude = ["kernel"]`, nên `gate --fast` (L0 trên
`v2/Cargo.toml`) **không** chạm `vifast-plat-common`. Test này chỉ chạy khi ai đó gõ tay
`--manifest-path v2/kernel/Cargo.toml`. Đây là lớp lỗi "test đỏ mà không cổng nào chạy" — cùng lớp
với `build-rev.sh --selftest` trước KT-134.

**Hai đường, Agent 0 chọn:**

| | (A) Sửa **test + chú thích** cho khớp `HART_STRIDE` | (B) Sửa **mã** để `HART_STRIDE` bỏ syscall stack |
|---|---|---|
| Đúng với hiện trạng | có — linker script đã cấp 32 KiB syscall stack (`linker.ld` chú thích KT-49/KT-66) | **không** — sẽ làm hẹp vùng per-hart so với linker script ⇒ `mmu::init` panic (nó kiểm khớp) |
| Rủi ro | thấp; chỉ sửa kỳ vọng + chú thích | cao; đổi bố cục bộ nhớ đã chạy |
| Vi phạm luật nào | "sửa kỳ vọng để test xanh" — **cần A0 cho phép rõ** | đổi bố cục bộ nhớ = đổi thiết kế |

**Đề xuất của Agent 1 (không tự quyết)**: **(A)**, kèm hai việc bắt buộc:
1. Sửa `smp.rs:23` thành `GUARD + STACK + TRAP + SYSCALL == HART_STRIDE` **và** sửa chú thích
   `smp.rs:10-11` — sửa **cả hai** để không còn chỗ nào ghi công thức cũ.
2. Thêm **cổng** chạy L0 của workspace kernel (`cargo test --manifest-path v2/kernel/Cargo.toml
   -p vifast-plat-common -p vifast-virtio`) vào `gate --fast`, để lớp lỗi này không tái diễn. Đây
   là **thêm phép đo**, không nới ngưỡng.

**Trạng thái**: **KHÔNG** sửa trong đêm (B1: Agent 1 được *từ chối* đổi kỳ vọng, không được tự đổi).
