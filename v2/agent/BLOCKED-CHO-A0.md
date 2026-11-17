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

## B4 · `cd` / `pwd` có thuộc “tương đương v1” không? (mở 2026-11-17, từ review KT-77a)

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

## B6 · build-inputs của D2-127 bao gồm `v2/agent/JOURNAL/**` ⇒ mọi phép đo tự vô hiệu (mở 2026-11-17, TRƯỚC KT-89)

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
