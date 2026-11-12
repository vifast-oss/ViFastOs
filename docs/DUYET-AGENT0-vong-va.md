# Duyệt của Agent 0 cho gói GUI-AGENT0.md (vòng vá 2026-11-12)

Người viết: Agent 0. Người nhận: Agent 1 (áp errata), sau đó người điều phối (ghi duyệt, commit).
Phạm vi: trả lời 3 điểm Agent 1 hỏi, liệt kê errata bắt buộc trước khi giao Agent 2, và chốt cách ghi duyệt.

---

## 0. Kết luận

1. Gói vá **đạt về hướng**: QD được áp gần đủ, hai QUEUE tự chứa, sổ lỗi v1 đầy đủ, EBNF tự chứa đã thành hình. Agent 1 làm tốt.
2. **Chưa ghi APPROVED dưới KT-04d.** KT-04d là duyệt `abi.toml` + 20 kịch bản, mà các thứ đó **chưa tồn tại** (KT-04a/b/c chưa chạy). Duyệt tài liệu thiết kế là việc khác, ghi bằng **D2-25** (mục 5).
3. Có **4 lỗi phải sửa trước khi giao** (mục 1 điểm 1, E2, S1, L-QUEUE vòng phụ thuộc ST-02a), cùng khoảng 20 errata nhỏ. Tất cả là sửa văn bản, không đổi kiến trúc.
4. Khi Agent 1 áp xong errata và chạy lại CHECK, người điều phối ghi duyệt theo mục 5 **mà không cần gửi lại Agent 0**.

---

## 1. Ba điểm Agent 1 hỏi

### Điểm 1. ST-02a "duyệt + hoàn thiện + golden": **không đồng ý cách này, sửa khác**

Vấn đề: ST-02a đang ở trạng thái `REVIEW`, nên `agentctl next` bỏ qua, **không ai làm phần golden**. Đồng thời tiêu chí "mọi khối mã trong tài liệu parse được" cần parser, mà parser là ST-05 lại bị chính ST-02a chặn. Đây là **vòng phụ thuộc**.

Quyết định:
- SPEC-lex.md + Phụ lục A do Agent 1 soạn, nên **người duyệt là Agent 0, ngay vòng này** (qua errata ở mục 3). Không còn bước "Agent 2 duyệt".
- ST-02a sau khi áp errata và ghi APPROVED thì **DONE**.
- 60 golden token chuyển thành **bước test đỏ đầu tiên của ST-04**.
- 30 golden biểu thức chuyển thành **bước test đỏ đầu tiên của ST-05**.
- Cổng "doc-examples parse" bật ở **ST-09** (khi đã có parser).

### Điểm 2. KT-04c chạy 5 ca trên mock: **không đồng ý, sửa khác**

Mục đích của KT-04c là kiểm **ABI có đủ để viết kịch bản không**, không phải kiểm kịch bản chạy đúng. Mock CNode/Untyped cũng không chạy được call/reply, suspend, exit, donation, nên 5 ca mock tốn công mà không chứng minh gì.

Quyết định:
- KT-04c = 20 tệp `.scn` + **kiểm tĩnh**: mọi bước tham chiếu syscall/method/mã lỗi có thật trong `abi.toml`, đủ tham số theo chữ ký. Kịch bản nào không viết được thì ghi "thiếu ABI" vào BLOCKED.
- KT-11 chạy các ca thuộc Untyped/CNode. Các ca còn lại chạy khi tới K1b, K1c.

### Điểm 3. `Tcb.ReadState.exit_code`: **đồng ý**, kèm một sửa quan trọng về Exit

Kernel lưu `exit_code: Option<u64>` trong TCB khi gọi `Exit`; `Resume`/`Configure` xoá nó. Lý do giữ: nếu không có fault_ep, hoặc procsrv khởi động lại, mã thoát vẫn đọc được.

**Sửa ngữ nghĩa Exit (E2 bên dưới, bắt buộc):** "gửi fault dạng Send không chờ" là sai nếu procsrv chưa `Recv`: Send trên endpoint là **chặn**, còn NBSend thì **làm mất** sự kiện thoát. Ngữ nghĩa đúng, không cần trạng thái mới:
- `Exit(code)`: lưu `exit_code`; nếu có fault_ep thì TCB thành `BlockedOnSend { ep: fault_ep, call: false }` mang `FaultKind::Exit{code}`; khi procsrv nhận thì TCB thành `Inactive`.
- Không có fault_ep thì `Inactive` ngay.
- `kill` khi TCB đang chờ giao Exit: `Suspend(Abort)` huỷ message, `exit_code` vẫn đọc được qua `ReadState`.
- Thêm kịch bản "Exit khi procsrv chưa Recv" vào 20 ca KT-04c.

Hai điểm nhỏ Agent 1 nêu: (a) Exit không có Reply: đồng ý; (b) cargo-deny là kế hoạch trong KT-03/ST-01: đồng ý.

---

## 2. Errata tài liệu ViFastOs v2 (`docs/v2/ViFastOs-v2-thiet-ke.md`)

| Mã | Chỗ | Sửa |
|---|---|---|
| E1 | §4.7 WaitOnAddress | **Xoá dòng còn sót** "Hàng chờ nằm trong **header Frame** ... [GT: chi phí với Frame 2 MiB...]". Nó mâu thuẫn với bảng băm vừa thêm |
| E2 | §4.4, §4.6 (fault), §4.21 dòng 1 và 14, bảng Exit/kill/wait, §6.4 | Ngữ nghĩa Exit như mục 1 điểm 3. Dòng Send ghi "fault `Exit`: giao bằng hàng gửi của fault_ep, không có Reply" (bỏ chữ "không chờ") |
| E3 | §4.4 enum + §4.5 bảng | `Sleeping` thành `Sleeping { reason: SleepReason }` với `SleepReason = Timeout \| Budget`; bảng ngân sách dùng đúng tên này |
| E4 | §4.21 đoạn đóng băng, D2-15 | Quy ước thanh ghi: **đóng băng riscv64 ngay**; aarch64, x86_64 đóng băng khi port (K4', K5'), chỉ là thêm, không đổi riscv64 |
| E5 | §11.6 | Bỏ trích dẫn "(QD §5.3)" sau "tối đa 3 QEMU"; QD không nói điều này. Ghi `[TK]` |

---

## 3. Errata ViScript (`ViScript-thiet-ke.md`, `SPEC-lex.md`)

### 3.1 Thiết kế

| Mã | Chỗ | Sửa |
|---|---|---|
| S1 | §22 và `viscript/agent/DECISIONS.md` | **Số VD lệch nhau giữa hai nơi** (ví dụ VD-12 là "SPEC-lex nguồn duy nhất" ở tài liệu nhưng là "thụt lề" ở DECISIONS). Lấy **DECISIONS.md làm chuẩn**, §22 chép lại nguyên. Bổ sung các mục chỉ có ở tài liệu: **VD-23** SPEC-lex + Phụ lục A là nguồn từ vựng/cú pháp; **VD-24** shell sở hữu tty, con nhận pipe; **VD-25** `from json` không tham số trả `Json`, `take`/SIGPIPE không phải lỗi, `\| complete`; **VD-26** consent và cache dùng hash closure. Sửa **VD-02** "cùng AST" thành "cùng HIR". Sửa các chỗ "sửa bởi VD-xx" ở bảng Q1..Q8 cho khớp |
| S2 | §21 Q6 | "bằng chứng nhận" là lỗi; trả lại "điều khoản **bằng sáng chế** (patent grant)" |
| S3 | Đầu tài liệu | Bỏ link tới `review/work/PATCH-NOTES-viscript.md`. Tài liệu trong `docs/` không trỏ vào `review/`. Nội dung cần giữ thì đưa vào §21.1 |
| S4 | §11.2 | `net: cap[Net] @ "api.example.com"` dùng chuỗi thường; host không phải path. `cap[Dir]` dùng `p"..."`. Kiểm loại ở pha kiểm kiểu |
| S5 | §9.5 | Fence `extern cmd git(...)` vẫn nhãn `python`, đổi thành `vis` |

### 3.2 Phụ lục A (EBNF)

| Mã | Sửa |
|---|---|
| S6 | `SIZE`, `DURATION`, `PERCENT` là **terminal một token** (khớp SPEC-lex). Bỏ các luật `size_lit := INT SIZE_UNIT`, `duration_lit := INT DUR_UNIT (...)`, `percent_lit` |
| S7 | Range pattern mở trên (đang dùng `{age: 18..}`): `range_pattern := atom_pattern [ '..' [atom_pattern] \| '..=' atom_pattern ]` |
| S8 | `NEWLINE`: chỉ giữ luật "dòng **bắt đầu** bằng `\|>` ở độ sâu 0 thì nối". Bỏ vế "dòng trước kết thúc bằng `\|>`" (SPEC-lex và §2.2 chỉ có một luật) |
| S9 | Định nghĩa riêng `break_stmt := 'break' NEWLINE` và `continue_stmt := 'continue' NEWLINE` (hiện `stmt` gọi `continue_stmt` mà không định nghĩa) |
| S10 | Ghi chú ràng buộc: lambda dạng `fn(...) -> T:` **nhiều dòng chỉ được ở cấp câu lệnh** (vế phải `let`/`var`/gán/`return`). Trong ngoặc INDENT bị tắt nên chỉ dùng dạng `=>` hoặc `fn` một dòng |
| S11 | `.vish`/REPL: thêm `shell_let := 'let' IDENT [':' type] '=' pipeline NEWLINE` và dùng trong `shell_item` (ví dụ `let big = ls ./photos \| where size > 5mb` có vế phải ở chế độ lệnh) |

### 3.3 SPEC-lex

| Mã | Sửa |
|---|---|
| L1 | Dòng `PATH`: bỏ ghi chú lửng "không nội suy? (QD...)", chốt **có escape, có nội suy** |
| L2 | `0b`/`0x` đứng trước chữ số hợp lệ là tiền tố cơ số; `0b` không theo sau bởi `[01]` là `SIZE(0,b)`. Thêm golden `0b101 => INT(5)` và `0b => SIZE(0,b)` |
| L3 | Golden `50% => !VS0103` sai: lexer cho `INT(50) PUNCT(%)`, lỗi xảy ra ở **parser** kèm gợi ý "dùng `50pct`". Sửa golden lex; thêm ca này vào 30 golden biểu thức của ST-05 |
| L4 | Golden `set{1,2}` mâu thuẫn luật: phải ra token riêng `SET_LBRACE`. `set {1}` mới là `IDENT(set) PUNCT({)` |
| L5 | Xoá golden `if x:  y => !VS0109` (khối một dòng có hai dấu cách không phải lỗi thụt lề). Thay bằng ca thật: thân khối thụt 4 rồi một dòng thụt 6 trong tệp đơn vị 4 |
| L6 | Lexer phi ngữ cảnh: `@` luôn là `PUNCT`; lỗi "dùng `@` sai chỗ" chuyển sang parser. Bỏ `@` khỏi VS0110 (giữ `^` vì lexer chế độ lệnh có biết đầu stage) |
| L7 | `PERCENT` nhận cả FLOAT (`12.5pct`), khớp tài liệu §2.4 |
| L8 | Đủ **đúng 60** golden sau khi sửa L2..L5; bộ này là đầu vào test đỏ của ST-04 |

---

## 4. Errata QUEUE, DECISIONS, AGENTS

| Mã | Tệp | Sửa |
|---|---|---|
| Q1 | `viscript/agent/QUEUE.md` | ST-02a: tiêu chí = "áp errata mục 3 + APPROVED"; **không phụ thuộc ST-01**; sau APPROVED là DONE. ST-04 thêm "60 golden token đỏ trước"; ST-05 thêm "30 golden biểu thức đỏ trước"; ST-09 giữ cổng doc-examples parse |
| Q2 | `v2/agent/QUEUE.md` | **KT-08** phụ thuộc **KT-02** (sim host không cần boot QEMU); tiêu chí: harness + store giả + scheduler 1 hart + kịch bản rỗng tất định theo seed, chưa cần ABI. **KT-09** phụ thuộc KT-08 **và KT-05**; kịch bản end-to-end Retype→Configure→Call/Reply chuyển sang KT-09. **KT-05** phụ thuộc **KT-04d**. **KT-12** phụ thuộc **KT-02** |
| Q3 | `v2/agent/QUEUE.md` | KT-04c theo mục 1 điểm 2 (kiểm tĩnh, 21 kịch bản gồm "Exit khi procsrv chưa Recv"); KT-11 tiêu chí "chạy các ca Untyped/CNode của KT-04c" |
| Q4 | `v2/agent/QUEUE.md` Khối A | SPDX theo loại tệp (giống AGENTS §7), bỏ câu "header `//` cho mọi tệp mã nguồn mới" |
| Q5 | `v2/agent/DECISIONS.md` | D2-24 và bảng Phần IV bị mất công thức (còn "N theo  (p..."), ghi chữ thường: `n ≥ 3/p`. CHECK báo "D2 26/24": rà trùng dòng, phải đúng 24 mục trước khi thêm D2-25 |
| Q6 | `AGENTS.md` | Bỏ "SA-A, SA-B, SA-C" (tên subagent nội bộ). Không ghi cứng số tệp lưu trữ (đang lệch 101/104/105); ghi "toàn bộ báo cáo agent2 của v1" |
| Q7 | `v2/agent/QUEUE.md` | Thêm dòng ghi chú: **KT-04d chỉ duyệt khi có sản phẩm KT-04a/b/c**; duyệt tài liệu thiết kế không ghi ở đây |

---

## 5. Cách ghi duyệt (người điều phối làm, sau khi Agent 1 áp errata)

Điều kiện: Agent 1 áp đủ E1..E5, S1..S11, L1..L8, Q1..Q7 và chạy lại CHECK, thêm 4 kiểm:
1. Không còn chuỗi "header Frame" trong `docs/v2`.
2. Danh sách VD-01..VD-26 giống hệt nhau ở `ViScript-thiet-ke.md §22` và `viscript/agent/DECISIONS.md`.
3. Không còn link `review/` trong `docs/`.
4. `v2/agent/DECISIONS.md` có đúng D2-01..D2-25.

Khi CHECK xanh, người điều phối ghi:
- `v2/agent/DECISIONS.md` thêm: **D2-25** Tài liệu `docs/v2/ViFastOs-v2-thiet-ke.md` bản vá 2026-11-12 được Agent 0 duyệt, kèm errata `DUYET-AGENT0-vong-va.md`.
- `viscript/agent/QUEUE.md`, ngay dưới ST-02a: `APPROVED 2026-11-12 Agent 0` (SPEC-lex + Phụ lục A sau errata).
- **Không** ghi gì dưới KT-04d.

Sau đó commit một lần (`[KT-00] vòng vá sau review + duyệt Agent 0`), lưu tệp này vào `docs/` cạnh QD, rồi giao Agent 2.

Trạng thái mở vòng 1 cho Agent 2:
- OS READY: KT-01, KT-02, KT-04a, KT-T26, KT-R1; sau KT-02 mở thêm KT-03, KT-06, KT-08, KT-12.
- ViScript READY (chỉ khi OS hết READY, D2-20): ST-01, ST-02b, ST-10; ST-04 mở ngay vì ST-02a đã DONE.
- Lần duyệt tiếp theo của Agent 0: **KT-04d** (abi.toml + 21 kịch bản + bảng ánh xạ).

---

## 6. Tin nhắn ngắn gửi Agent 1

> Agent 0 duyệt hướng, chưa duyệt KT-04d (chưa có abi.toml). Ba điểm: (1) không giữ ST-02a ở REVIEW: anh duyệt SPEC-lex + Phụ lục A ngay qua errata, golden chuyển vào ST-04/ST-05; (2) KT-04c kiểm tĩnh, bỏ mock 5 ca; (3) đồng ý exit_code, nhưng Exit phải đi qua hàng gửi của fault_ep (BlockedOnSend) để không mất sự kiện. Áp errata E1..E5, S1..S11, L1..L8, Q1..Q7 trong `DUYET-AGENT0-vong-va.md`, chạy lại CHECK với 4 kiểm ở mục 5. Xanh thì người điều phối ghi D2-25 và APPROVED dưới ST-02a, commit, giao Agent 2. Không cần gửi lại anh.
