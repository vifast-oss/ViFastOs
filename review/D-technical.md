# D — Phản biện kỹ thuật hai tài liệu thi công (ViFastOs v2 + ViScript)

Người phản biện: kiến trúc sư HĐH (vai khó tính). Ngày: 2026-11-12.
Đầu vào: `review/src/ViFastOs-v2-thiet-ke.md` (986 dòng), `review/src/ViScript-thiet-ke.md` (984 dòng),
đối chiếu `review/src/ViFastOs-dossier.md` (hồ sơ v1, số đo thật + lỗi đã biết).
Không sửa tệp nguồn. Không dùng web. Mọi dẫn chứng là `file:dòng`.

Quy ước trích dẫn: **V2** = `ViFastOs-v2-thiet-ke.md`, **VS** = `ViScript-thiet-ke.md`, **DS** = `ViFastOs-dossier.md`.

---

## 0. Phán quyết

**Hướng kiến trúc: ổn.** Capability kiểu seL4, object trong Untyped, sim tất định + mô hình tham chiếu,
kịch bản chạy hai nơi — đây là cách đúng để viết lại một kernel đã có 6 lỗi gốc (V2:53-64), và nó sửa
đúng những chỗ v1 chết (DS:1922-1950).

**Như viết: KHÔNG thi công được.** Không phải vì thiếu chi tiết vặt, mà vì ba lỗi cấu trúc:

1. **Job control không biểu diễn được** trên máy trạng thái TCB hiện có và trên ABI hiện có (điểm 1).
   Đây là chỗ *chắc chắn vỡ* đầu tiên, và nó vỡ ngay ở U1 — tức sau khi ABI đã "đóng băng".
2. **"ABI" trong tài liệu là danh sách tên, không phải ABI** (điểm 6): không chữ ký, không quy ước
   thanh ghi, không version, thiếu method sống còn (`SetReplyObject`, `ReadState`/`Wait`, exit). Chốt
   ở K0 nghĩa là chốt phần rẻ và để phần đắt cho Agent 2 tự phát minh *sau* khi bị cấm đổi spec.
3. **Khối lượng vs năng lực**: MCS đầy đủ + objmem/Miri + CSpace 2 cấp + 3 arch + Rust std + VFS
   journal + một ngôn ngữ mới (lexer 2 chế độ + typechecker + bytecode VM + LSP), với **một** agent,
   **3 giờ/task** (V2:887). Điểm 9 ước lượng chỗ trượt; không phải chỗ nào cũng trượt, nhưng 4 chỗ
   chắc chắn.

**Ba câu chốt:** (a) job control phải thiết kế lại *trước* K0, không phải ở U1; (b) D-22 phải thu hẹp
phạm vi đóng băng + thêm version, nếu không nó là bẫy; (c) cắt cơ chế MCS và 2/3 arch khỏi 90 ngày đầu.

---

## 1. Bảng phát hiện (theo mức)

| # | Mức | Vị trí | Một dòng |
|---|---|---|---|
| F1 | **P0** | V2:222-235, 414, 429 | ThreadState không có trạng thái "bị dừng"; `TcbInfo`/`TcbWait` bị bỏ, không có `ReadState`/`Wait` → `jobs`/`wait`/`fg` không cài được |
| F2 | **P0** | V2:323 + 414 + 714 | `Suspend` khi đang chờ IPC: `Restart` phát lại syscall ⇒ gửi lặp message (đúng lớp lỗi T31 của v1); không có ngữ nghĩa "huỷ IPC" |
| F3 | **P0** | V2:229, 241, 260 | `Call` cần Reply object nhưng TCB không có trường/không có `SetReplyObject` → đường nóng nhất của IPC không đặc tả được |
| F4 | **P0** | V2:408-427, 935, 955 | "ABI" không có chữ ký/thanh ghi/version; T04 một task 3h cho ~80 entry point; freeze trước khi có sim |
| F5 | **P1** | V2:174-177, 260-262 | `get2_mut` không đủ cho IPC (≥4 object); `place` + `&mut` vào `Vec<u8>`/physmap sẽ đụng Miri/Stacked Borrows → phải đổi shape API sau freeze |
| F6 | **P1** | V2:272, 168 | Hàng chờ WaitAddr "trong header Frame": ObjHeader 24 B không chứa được; nếu là header *trang* thì user ghi được → hỏng con trỏ kernel |
| F7 | **P1** | VS:356, 866, 143, 202 | panic "dừng task hiện tại" nhưng không định nghĩa task/không unwinding/không mã lỗi; tràn & chia 0 là lỗi *dữ liệu* nhưng bị coi là lỗi lập trình (VS:6) |
| F8 | **P1** | VS:125, 202, 261, 483 | literal `50%` đụng toán tử `%`; `%` không có ngữ nghĩa dấu; ví dụ `cpu > 50%` không kiểm kiểu được |
| F9 | **P1** | VS:248 vs 869 | `\|` vẫn dùng cho or-pattern, mâu thuẫn Q8 "chỉ pipe"; thiếu `bit_not`; `shr` không định nghĩa số học/logic |
| F10 | **P1** | V2:280 vs 353 | push migration cần đọc trạng thái hart khác, đúng thứ §4.8 từ chối ở work-stealing, và vi phạm luật 3 |
| F11 | **P2** | VS:104 vs 628-635 | tab là *lỗi từ vựng* nhưng `vis fmt` được giao "tự sửa" — fmt chạy trên CST, không lex được thì không sửa được |
| F12 | **P2** | V2:786 vs 646, 762 | boot đầu tiên ở K3 nhưng cổng ngày cần QEMU; gate --fast ở K0 rỗng nghĩa (L1 cần sim) |
| F13 | **P2** | VS:100 vs 908 | từ khoá thiếu `const`; EBNF thiếu `assign/lambda/field_updates/types`; `[types]` nhập nhằng với index |
| F14 | **P2** | V2:212-213 | slot cap 32 B không đủ (objref 16 + badge 8 + mdb 8 = 32, còn kind/rights/depth) → ≥40 B |
| F15 | **P2** | VS:549 | consent theo hash *tệp chính*, bỏ qua closure phụ thuộc → thay `use ./util` là chạy với quyền cũ |

---

## Điểm 1 — Job control "không dùng signal"

### Dẫn chứng
- V2:510 (§6.4): process server cung cấp `wait`, `kill`, danh sách tiến trình; "**job control không dùng
  signal** (huỷ, tạm dừng, tiếp tục qua IPC)". V2:576 giao cho U1; V2:578 nói rõ ViScript cần
  "`kill/wait/list`".
- ABI: V2:414 liệt kê method TCB — có `Resume`, `Suspend`; **không** có `ReadState`, `Wait`, `Exit`,
  `SetReplyObject`, `Cancel`. V2:429 bỏ `TcbInfo` (và `TcbWait` của v1 biến mất, không nói).
- V2:222-235: 11 trạng thái, `Stopping` chỉ là "chờ hart khác dừng mình (finalise, migrate)" (V2:233).
- V2:323: cơ chế làm tiếp = trả `Restart` để platform "quay lại chính lệnh syscall (đặt lại ip)".
- V2:714: sim **có** tiêm lỗi "Suspend khi đang chờ" — tức tác giả biết ca này, nhưng không định nghĩa kết quả.
- VS:721: job `&`, `jobs`, `fg`, `bg`, `kill`; "POSIX dùng process group; ViFastOs qua process server".
  VS:722: "Ctrl-C huỷ pipeline qua token huỷ lan theo stream và scope". VS:726: "ViShell **không tự là
  terminal**". VS:584: Terminal trait chỉ có `term_size`, `term_raw_mode`, `term_read_key`.
  VS:464: mã thoát khác 0 là lỗi pipeline, `| complete` trả `{stdout, stderr, code}`.

### Phân tích

**(a) Máy trạng thái không biểu diễn được "đang dừng".** Ctrl-Z một tiến trình đang `Running` thì nó
thành gì? `Inactive`? `Suspend` một tiến trình đang `BlockedOnRecv{ep}` thì `ep`/`reply` để đâu? Nếu
giữ, invariant V2:239 ("TCB ở trong hàng của X **khi và chỉ khi** trạng thái mang X") bị vi phạm, vì
TCB vẫn nằm trong hàng endpoint nhưng trạng thái không còn mang `ep`. Nếu bỏ, `Resume` không khôi phục
được — thread tỉnh dậy sau `Recv` mà không có message. Không có đường thứ ba trong enum. Đây là lỗi
*biểu diễn*, không phải lỗi thiếu hàm: mọi cách cài đặt đều phải thêm trạng thái hoặc thêm cờ.

**(b) Không có thao tác huỷ IPC.** ABI chỉ tạo ra `Cancelled` ở một chỗ: finalise object (V2:195-199).
Không có `Cancel` trên Endpoint/Reply/TCB. Hệ quả cụ thể: client đang `BlockedOnReply` bị Suspend; server
giữ Reply và gọi `Send(reply)`. Caller không còn `BlockedOnReply` ⇒ `Send` phải trả lỗi. Vậy một client
bị Ctrl-Z làm **hỏng đường xử lý lỗi của server** (server coi request hợp lệ là thất bại). Nếu thay vào
đó `Send` thành công và caller thành "Ready nhưng bị dừng" — trạng thái đó không tồn tại trong enum.
Cả hai nhánh đều không cài được.

**(c) Resume mà phát lại syscall = gửi lặp message.** V2:323 định nghĩa cơ chế "làm tiếp" bằng cách đặt
lại ip để chạy lại syscall. Đó là cơ chế *duy nhất* có trong thiết kế để một TCB bị dừng giữa syscall
tiếp tục được. Với `Recv` thì tạm chấp nhận; với `Call` thì **gửi lại request** (server xử lý hai lần);
với `ReplyRecv` thì reply hai lần. Đây đúng lớp lỗi v1 đã trả giá: T31 "PAGER2 nhận message lặp"
(DS:1694-1706) và bài học "mọi `let _ = send(...)` là chỗ giấu lỗi" (DS:650-654). Nếu Resume **không**
phát lại, caller tỉnh dậy ngay sau `Call` mà không có reply ⇒ treo vĩnh viễn. Thiết kế hiện tại chỉ có
hai lựa chọn, cả hai đều sai; lựa chọn đúng là lựa chọn thứ ba chưa được viết: **`Call` trả `Cancelled`**.

**(d) Không có primitive quan sát/chờ.** `jobs` cần biết "đang chạy / đang dừng / đã thoát", `wait` cần
một sự kiện. ABI không có `ReadState`, không có `Wait`, không có death-notification. procsrv có thể tự
phát minh giao thức exit (con `Send` mã thoát tới procsrv rồi tự `Suspend`), nhưng (i) nó không phân biệt
được "blocked" với "stopped" để in `jobs`; (ii) `kill` = xoá cap → finalise, mà finalise TCB đang
`Running` ở hart khác cần handshake `Stopping` (V2:195, 233) — procsrv phải biết handshake xong mới báo
`wait` trả về, và không có sự kiện nào cho biết điều đó. Không có primitive thì mọi thứ còn lại là
polling hoặc tranh chấp.

**(e) Không ai giữ bàn phím.** Đây là chỗ tài liệu *tự nói mình không có terminal*: ViShell "không tự là
terminal" (VS:726), Terminal trait chỉ là nguồn phím thô (VS:584), không có termios, không có
foreground process group, không có line discipline, không có ai sở hữu "phiên". Hai mô hình đều không
chọn được:
- Nếu **con** sở hữu bàn phím (mô hình POSIX): ViShell đang `wait` không thấy Ctrl-C; trừ khi tty tự
  sinh tín hiệu — mà không có tty.
- Nếu **shell** sở hữu bàn phím và chuyển tiếp byte: con không thể vào raw mode, không resize, không
  chuột ⇒ không có `vim`/`less`/REPL tương tác; và shell phải phân biệt "byte cho con" với "Ctrl-C cho
  nhóm", tức lại cần termios.
Tài liệu không chọn, nên cả Ctrl-C lẫn chương trình toàn màn hình đều không cài được *cùng lúc*.

**(f) Không có hợp đồng tháo pipeline.** Không có mã lỗi `BrokenPipe` trong V2:383. Không nói ai kill
các stage còn lại, theo thứ tự nào, chờ bao lâu, mã thoát của pipeline bị huỷ là gì. `| complete`
(VS:464) cần `{stdout, stderr, code}` — một pipeline bị Ctrl-C trả code gì? Không định nghĩa.

### Kết luận: **THIẾU (nặng) — bất khả thi như viết**, không phải bất khả thi về nguyên tắc.

Thiếu đúng 5 thứ: (1) trạng thái dừng trong máy trạng thái; (2) ngữ nghĩa huỷ IPC khi Suspend;
(3) primitive chờ/quan sát trạng thái + exit; (4) chủ sở hữu terminal + foreground group; (5) hợp đồng
tháo pipeline + mã lỗi. ABI §4.21 **không đủ**: có suspend/resume *tên* nhưng không có ngữ nghĩa, không
có kill nguyên tử (chỉ có xoá cap), **không có wait**, **không có cancel IPC**.

### Đề xuất sửa (văn bản cụ thể để dán vào spec)

1. **Trạng thái**: thêm cờ trực giao, không thêm 11 trạng thái mới:
   `ThreadState = Base(Ready|Running|BlockedOn*|Sleeping|Inactive|Dead) + flags{suspended: bool, restart: bool}`.
   Bảng chuyển tiếp bổ sung: `stop(t)` khi `BlockedOn*` → giữ nguyên `blocked_on` (để invariant V2:239
   vẫn đúng), set `suspended`; khi `Ready` → set `suspended`; khi `Running` (cùng hart) → `Inactive +
   suspended`. `resume(t)` xoá cờ; nếu `restart` thì chạy lại syscall, ngược lại trả kết quả đã ghi.
2. **Suspend có chế độ**: `Tcb.Suspend(mode: Abort | Defer)`.
   `Abort` (mặc định cho job control): syscall đang chờ **trả `Cancelled`** vào thanh ghi kết quả, ip
   tiến qua syscall, TCB thành `Ready + suspended`. `Defer`: chỉ hợp lệ cho thao tác dài đã có `Restart`
   (V2:322-323) — **cấm** dùng cho Call/Send/Recv/ReplyRecv.
3. **Quan sát + chờ**: thêm `Tcb.ReadState -> {state, stop_reason, exit_code}` **hoặc** khôi phục
   `TcbWait` của v1 + `Tcb.SetDeathNotification(ntfn, badge)` (finalise signal). Định nghĩa **exit là
   fault**: `FaultKind::Exit{code}` gửi tới `fault_ep`, procsrv coi là chết. Không cần API mới cho exit.
4. **Terminal**: `term.vidl` với `cap[Term]`: `get_termios/set_termios{cooked,echo,isig}`, `set_fg(group)`,
   `read/write`, `resize`, `poll_hup`. Quyết định cho v2: **shell sở hữu tty**, con nhận **pipe**
   (`cap[File]` đôi), không nhận `cap[Term]`; `vim` để sau với tty server riêng. Ghi rõ trong VS §15.
5. **Nhóm + huỷ**: `procsrv.CancelGroup(gid, how)` và `procsrv.WaitGroup(gid) -> [status]` (RPC, không
   phải syscall). procsrv giữ bảng `gid -> [tcb cap]`, huỷ **từ stage cuối về đầu**, chờ tối đa `T`,
   rồi finalise. Trả `{code: u8, how: Exited|Cancelled|Faulted|Stopped}`.
6. **Mã lỗi**: thêm `BrokenPipe`, `Stopped`. `| complete` trả `{code, how, cancelled}`.
7. **Sửa V2:429**: bỏ `TcbInfo` là sai — `jobs`/`ps` cần nó; nếu muốn giảm syscall thì để nó thành
   method `Tcb.ReadState`, nhưng phải tồn tại từ K1.

---

## Điểm 2 — `xs[i]` panic trong shell

### Dẫn chứng
- VS:356 (§6.4): "panic dừng **task hiện tại**, in vết ngăn xếp có vị trí nguồn. **Không bắt được trong
  code thường**; chỉ ranh giới task/shell bắt để báo cáo."
- VS:866 (Q5): `xs[i]` panic, `xs.get(i) -> T?` là đường an toàn.
- VS:143: tràn số là panic. VS:202: chia 0 là panic. VS:348: `else` được phép thoát bằng `panic`.
- VS:6/VS:83: "panic **chỉ cho lỗi lập trình**, không cho lỗi môi trường"; "Lỗi là giá trị".
- VS:511: huỷ lan theo cây `scope`, điểm huỷ tại mỗi `await`. VS:376: RC + CoW, không GC truy vết.
- Không có `defer`/`finally`/`Drop` trong danh sách từ khoá (VS:100) hay trong EBNF (VS:906-935).

### Phân tích

**"Task hiện tại" là gì trong ViShell? Không định nghĩa.** §15 (VS:714-716) nói REPL "đọc dòng, phân
tích, kiểm kiểu, chạy, hiển thị" — lệnh chạy trong VM của shell, trên luồng của shell, trong heap của
shell. Trừ khi mỗi lệnh được bọc trong một task/arena riêng, "ranh giới shell" = cả phiên. Vậy một lỗi
gõ (`$xs[10]`) giết cả phiên: mất biến, mất cwd, mất bảng job, mất lịch sử chưa ghi. Với một shell là
*sản phẩm*, đây là hành vi không chấp nhận được; với một ngôn ngữ *script*, nó chấp nhận được. Tài liệu
dùng chung một câu cho cả hai.

**Không có cơ chế dọn dẹp.** Không `defer`, không `Drop`, không `finally`; RC giải phóng theo lối bình
thường, nên panic giữa biểu thức làm rò **toàn bộ heap của task** (VS:376: RC, không GC). Trong REPL
sống lâu, mỗi lệnh hỏng rò một ít. Ngoài ra panic không dọn: terminal đang raw mode, tiến trình con đang
chạy, file đang mở, cap đang giữ, temp file. `scope` (VS:511) chỉ dọn task con khi *thoát bình thường*
hoặc bị huỷ — không nói panic có huỷ task con hay không.

**Không có mã lỗi/status.** `| complete` cần `{stdout, stderr, code}` (VS:464). Panic không sinh ra
`Error` (nó không phải giá trị — VS:356 nói không bắt được), nên shell phải tự bịa code. Không định
nghĩa.

**Mâu thuẫn nguồn gốc:** tràn số và chia 0 là panic (VS:143, 202) nhưng chúng phụ thuộc **dữ liệu**:
`total / count` với `count` lấy từ `ps`, hay `sum` của `size` trên thư mục lạ. VS:6 nói panic chỉ dành
cho lỗi lập trình. Vậy cùng một toán tử vừa là "lỗi lập trình" vừa là "lỗi môi trường" tuỳ dữ liệu —
định nghĩa không đứng được. Hệ quả thực tế: pipeline shell chết vì dữ liệu, đúng thứ mà "lỗi là giá trị"
sinh ra để tránh.

**`str` index không định nghĩa.** EBNF cho `postfix '[' expr ']'` trên **mọi** giá trị (VS:932), `str`
là UTF-8 bất biến (VS:145), và không có luật byte/char/grapheme. Với tiếng Việt/Nhật, `s[0]` hoặc panic
giữa ký tự, hoặc cắt một grapheme — đây sẽ là nguồn panic phổ biến nhất của dự án này. Chỉ số âm
(`xs[-1]`) cũng không định nghĩa.

### So với lựa chọn "trả lỗi"

`xs[i] -> T?` (mọi truy cập index trả Option) làm `for i in 0..xs.len(): xs[i]` phải `??`/`else` ở mọi
chỗ, và làm `where rows[i].size > 5mb` trong pipeline shell rất khó viết — chi phí cú pháp hằng ngày,
đổi lấy an toàn ở chỗ hiếm. Đó là lý do Q5 chọn panic, và lựa chọn đó **bảo vệ được** — nhưng chỉ khi
có ranh giới và có đường checked. Hiện tại thiếu cả hai.

### Kết luận: **THIẾU + mâu thuẫn nội tại.** Q5 không sai; phần bao quanh nó sai.

### Đề xuất sửa
1. **Ranh giới = lệnh, không phải tiến trình.** ViShell bọc mỗi lệnh trong một *task* có heap arena
   riêng; panic giải phóng arena, in trace + span, đặt `status = 101`, **giữ** biến/cwd/job/lịch sử.
   Ghi vào VS §15 (bảng REPL) và §6.4.
2. **Thêm `defer`** (một dòng trong EBNF: `stmt := ... | 'defer' expr`) hoặc quy định "tài nguyên do
   lệnh giữ là cap của *phiên*, không phải của task" để arena-free là đủ. Cách hai rẻ hơn.
3. **Thêm toán tử checked**: `xs[i]?` → `T?` (đối xứng với `?.` đã có ở VS:254). Giữ `xs[i]` panic.
4. **Cấm `str[i]`** (lỗi kiểu, có gợi ý `.chars()/.graphemes()/.byte_at(i)?`); `xs[-1]` = panic với
   thông điệp rõ. Ghi vào §3.5/§4.5.
5. **Tách lỗi số học theo ngữ cảnh**: `int` tràn/chia 0 = panic (giữ), nhưng closure trong
   `where/each/sort_by` nhận `--on-error=skip|abort` + bộ đếm lỗi theo dòng, báo ở cuối pipeline. Đây
   là thứ shell thật cần và rẻ (một cờ + một bộ đếm).
6. **`panic` có payload**: `panic(Error)` để ranh giới render được; `test fn` coi panic là fail.
7. Sửa VS:6 thành: "panic cho *bất biến bị vi phạm* (kể cả số học); lỗi dữ liệu phải đi qua `Result`".

---

## Điểm 3 — `int / int -> int`

### Dẫn chứng
- VS:202-204 (§3.6): `int / int -> int` cắt về 0; chia 0 panic; có `int.div_floor`, `int.checked_div`;
  lint khi chia **hai literal nguyên**.
- VS:143: `int` = i64, tràn là panic; `wrapping_add` là đường tường minh.
- VS:125: `50%` là literal kiểu `percent`. VS:261: `%` nằm trong nhóm toán tử số học.
- VS:483: `ps | where cpu > 50% | each p => p.kill()` — ví dụ flagship của chế độ lệnh.
- VS:173, 198: không chuyển đổi ngầm giữa số (ngoại lệ: literal nguyên ở vị trí `float`).

### Phân tích — 6 khuyết điểm, 2 trong đó là *sai*

1. **`%` không có ngữ nghĩa.** VS:261 liệt kê `%` nhưng không nói dấu: `-7 % 2` = `-1` (C/Rust) hay
   `1` (Python)? Chỉ có `div_floor`, **không** có `mod_floor`/`rem_euclid`. Với `duration`/`datetime`
   âm (múi giờ, `time.now() - since`), sai một đơn vị là sai một ngày trong báo cáo.
2. **Literal `50%` đụng toán tử `%` — nhập nhằng từ vựng.** `100%2` là `100%` + `2` (lỗi cú pháp) còn
   `n%2` là modulo, vì `n` không phải literal số. Cùng một ký tự, hai cách lex tuỳ ngữ cảnh *ký tự đứng
   trước*, không có luật khoảng trắng trong §2.4 hay §4.5. Đây là lỗi *thiết kế từ vựng*, phải sửa
   trước VT04.
3. **Lint quá hẹp.** "Chia hai literal" bắt `1 / 2` (mà lập trình viên đã cố ý) và bỏ sót 100% ca thật:
   `total / count`, `bytes / n`, `%` tính từ hai biến. Giá trị thực tế gần bằng 0.
4. **Ví dụ flagship không kiểm kiểu được.** `cpu > 50%` (VS:483) so sánh `float` với `percent` — VS:173
   cấm chuyển đổi ngầm, và không có luật `percent` so sánh với `float`. Hoặc `ps.cpu` phải có kiểu
   `percent` (không nói ở đâu), hoặc ví dụ này không chạy. Một ví dụ không chạy được trong tài liệu
   thi công là lỗi thiết kế, không phải lỗi chính tả.
5. **Không có số học cho `size`/`duration`/`percent`.** `5kb + 2kib`? `size * size`? `size / duration`
   (vận tốc)? `percent * size`? `datetime + 1d` qua DST? §3.1 khai 6 kiểu hệ thống nhưng chỉ có `int`
   và `float` có luật. Agent 2 sẽ tự phát minh → phá "một nguồn sự thật" (V2:80).
6. **`**` không có luật**: `2 ** -1`, `2 ** 63` (tràn i64 → panic?), `0.0 ** -1`. Không định nghĩa.

Ngoài ra `int/int -> int` tự nó **ổn**: nhất quán với kiểu tĩnh, không ngầm chuyển, và có
`checked_div`. Vấn đề không phải phép chia, mà là hệ sinh thái quanh nó.

### Kết luận: **SAI ở (2) và (4); THIẾU ở (1), (5), (6); (3) là thiết kế lint chưa đủ.**

### Đề xuất sửa — bảng ngữ nghĩa bắt buộc (dán vào §3.6)

| Biểu thức | Kiểu | Luật |
|---|---|---|
| `int / int` | `int` | cắt về 0; `/0` panic; có `div_floor`, `div_euclid`, `checked_div` |
| `int % int` | `int` | dấu theo **số bị chia** (C/Rust); `%0` panic; có `mod_floor`, `rem_euclid` |
| `size ± size` | `size` | tràn = panic; `size * int`, `size / int`; `size / size -> float` |
| `duration ± duration` | `duration` | `duration * int`, `duration / int`, `duration / duration -> float` |
| `datetime - datetime` | `duration` | `datetime ± duration -> datetime` (**theo UTC**, không theo lịch) |
| `percent * size` | `size` | `percent * int -> int`; `100% * n` |
| `int ** int` | `int` | mũ âm = lỗi kiểu; tràn = panic; có `checked_pow` |

Và:
- **Bỏ literal `50%`**, đổi thành `50pct` (hoặc `pct(50)`) — rẻ hơn nhiều so với mọi luật khoảng trắng,
  và không phá cú pháp nào khác. Nếu giữ `%`: bắt buộc "`%` là toán tử chỉ khi có khoảng trắng hai phía;
  `100%2` là lỗi từ vựng có gợi ý".
- **Đổi lint**: cảnh báo mọi `int/int` khi kết quả được so sánh/dùng với `float`/`percent`, cộng lint
  `percent-of-int-division`. Giữ lint literal nhưng hạ mức.
- **`percent` so sánh được với `float`** theo tỉ lệ (50% == 0.5), hoặc `ps.cpu: percent`. Chọn một, ghi
  vào §3.5 để VS:483 chạy được.

---

## Điểm 4 — Thụt lề 4 dấu cách, cấm tab

### Dẫn chứng
- VS:104: "thân thụt lề **4 dấu cách**. Tab bị cấm (**lỗi từ vựng**, `vis fmt` tự sửa)."
- VS:628-635: `lexer → parser → CST → … → vis-fmt` — fmt **sau** lexer, chạy trên CST.
- VS:106: thân một dòng sau `:` được phép. VS:105: trong `()[]{}` xuống dòng tự do.
- VS:779/VT07: vòng tròn "parse → fmt → parse cho AST bằng nhau"; VT05: "in lại CST đúng nguồn".
- VS:846/VT04: lexer 2 chế độ, thụt lề, literal, cấm tab, Unicode + NFC.

### Phân tích

**Chi phí thật** không phải lexer (khoảng 250-400 dòng + 40 test, chấp nhận được) mà là **sáu ca biên
không được định nghĩa**, mỗi ca là một tuần sửa lỗi của Agent 2:

1. **Dòng trống / chỉ có comment**: có sinh INDENT/DEDENT không? Comment ở cột 0 nằm trong một block có
   làm đóng block không? (Python giải quyết bằng tokenizer state; ở đây không nói.)
2. **Chuỗi nhiều dòng** `"""..."""` với nội dung ở cột 0 — không được đóng block. Chỉ đúng nếu lexer
   nuốt trọn token trước khi tính thụt lề; phải nói rõ.
3. **`cmd { ... }`**: thân dùng ngoặc nhọn (VS:420, 933) — thụt lề **không có nghĩa** trong đó. Nhưng
   VS:105 nói `{}` xuống dòng tự do, nên `cmd {` nhiều dòng là hợp lệ; phải nói rõ lexer tắt INDENT
   trong `{}` và bật lại sau `}`.
4. **Độ rộng**: 2 dấu cách là lỗi từ vựng hay hợp lệ? "4 dấu cách" đọc như bắt buộc tuyệt đối; nếu vậy,
   mọi code dán từ nơi khác đều chết ngay ở dòng đầu. Nếu không bắt buộc, luật không phải "4".
5. **REPL / dán nhiều dòng**: không định nghĩa ở đâu. Dán một hàm 10 dòng vào shell: dòng nào là tiếp
   nối? Khi nào block kết thúc? (Python cần một dòng trống.) Không có bracketed paste, không có
   `:paste`, không có "DEDENT ngầm ở cuối input".
6. **Tab**: xem dưới.

**Mâu thuẫn chặn đường:** VS:104 nói tab là **lỗi từ vựng** và **`vis fmt` tự sửa**. Nhưng fmt chạy trên
CST (VS:628-635); một tệp không lex được thì không có CST, nên fmt **không thể** sửa. Đây là lỗi logic
trong thiết kế công cụ, và nó đúng với mọi lỗi từ vựng khác. Sửa: tab là *diagnostic của lexer* (vẫn
tiếp tục lex, quy ước tab = 4), `vis check` báo lỗi, `vis fmt` sửa. Hoặc fmt có pre-pass thay tab trước
khi lex — nhưng phải nói ra.

**Có đáng không?** Có. Đối tượng là người viết Python; shell + script cùng ngôn ngữ; ngoặc nhọn sẽ đắt
hơn ở chỗ khác (dấu `;`, `{}` đụng record literal). Giữ thụt lề, nhưng luật phải là: *"đầu vào: thụt lề
nhất quán bất kỳ (lexer tự suy đơn vị từ cấp đầu tiên); đầu ra: luôn 4; tab = lỗi của `check`, được
`fmt` sửa"*. Như vậy dán code không chết, mà vẫn "một kiểu duy nhất" như Q3 muốn (VS:864).

**Rủi ro khi Agent 2 tự sinh code.** Code LLM sinh ra thường nhất quán 4 dấu cách, nên phần *sinh* ít
rủi ro. Rủi ro thật nằm ở ba chỗ khác:
- **Code dán từ tài liệu/web** (tab, 2 spaces, trộn) — chiếm phần lớn thời gian sửa của một ngôn ngữ
  thụt lề.
- **VT09**: "≥100 tệp lấy từ ví dụ trong tài liệu" (VS:851) — nhưng nhiều ví dụ **không phải ViScript
  hợp lệ**: VS:478/484/973 bắt đầu bằng `> ` (prompt, `>` là toán tử so sánh); VS:297/309/318 dùng `...`
  làm thân hàm (`...` không có trong ngữ pháp); VS:483 dựa vào `percent` so sánh `float` (điểm 3). Vậy
  bộ test không thể "lấy từ tài liệu", phải **viết lại** — và đó là một task riêng, không phải VT09.
- **fmt idempotence**: nếu lexer chuẩn hoá thụt lề, VT05 ("in lại CST đúng nguồn") và VT07 ("parse→fmt→
  parse") chỉ cùng đúng khi nguồn đã đúng 4 spaces. Phải nói rõ: CST giữ *độ rộng gốc* cho fmt, AST
  không.

### Kết luận: **ỔN về lựa chọn, SAI ở câu "tab là lỗi từ vựng + fmt sửa", THIẾU 5 ca biên + REPL/paste.**

### Đề xuất sửa
1. VS:104 → "Tab: lexer báo `VS0107`, quy ước 4, tiếp tục; `vis fmt` sửa; `vis check` coi là lỗi."
2. Thêm mục "Thụt lề — ca biên" với 6 luật: dòng trống/comment không sinh INDENT; comment cột 0 trong
   block không đóng block; chuỗi nhiều dòng nuốt trước khi tính cột; `{}`/`[]`/`()` tắt INDENT; DEDENT
   ngầm ở EOF và ở cuối mỗi input REPL; đơn vị suy từ cấp đầu, xuất luôn 4.
3. REPL: bracketed paste → parse cả buffer; dòng cấp 0 sau block = kết thúc block; `:paste` dự phòng.
4. VT09: tách thành "viết lại ≥100 tệp test hợp lệ" + "bộ chuyển ví dụ tài liệu → test" (bỏ `> `, thay
   `...`), và thêm vào cổng kiểm "mọi ví dụ trong docs phải qua `vis check`" — nếu không, tài liệu sẽ
   lệch mã như v1 (DS:1967-1987).

---

## Điểm 5 — Toán tử bit là hàm, `|` chỉ pipe

### Dẫn chứng
- VS:261 (§4.5): bit bằng hàm `bit_and`, `bit_or`, `bit_xor`, `shl`, `shr`; "để `|` dành cho pipe";
  nối hàm `|>`.
- VS:869 (Q8): "Hàm (`bit_or`...), không ký hiệu. Giải phóng `|` cho pipe **ở mọi chế độ**".
- VS:452 (§9.4 bảng): "`|` — chế độ lệnh: Pipe; chế độ script: **Không dùng** (bit OR là `bit_or`)".
- VS:248 (§4.3): pattern gồm "… khoảng `0..10`, **`|`**, guard `if`".  ← dùng `|` cho or-pattern.
- VS:916-935 (EBNF): `pipe_expr := or_expr ('|>' or_expr)*` — `|>` **có** trong chế độ script.
- VS:439/723: `^` là tiền tố "ép gọi chương trình ngoài".

### Phân tích
1. **`|` bị dùng lại cho or-pattern** (VS:248) — mâu thuẫn trực tiếp Q8 (VS:869) và bảng VS:452. Đây
   không phải lỗi chữ: nếu pattern dùng `|`, thì trong chế độ lệnh `ls | each x => match x.kind: A | B: 1`
   có hai cách hiểu, và bộ tách pipeline phải biết ngoặc. Không có luật nào nói điều đó.
2. **Bảng VS:452 sai ở cột script**: `|` không dùng, nhưng `|>` **có** dùng (VS:261, 921, 957-958,
   491). Đọc bảng sẽ tưởng chế độ script không có pipe.
3. **Thiếu `bit_not`**: 5 hàm, không có phủ định bit. Người dùng sẽ viết `bit_xor(x, -1)` (phụ thuộc bù
   hai, và với `int` i64 thì đúng nhưng không hiển nhiên). Phải thêm.
4. **`shr` nhập nhằng số học/logic**: với `int` i64, `shr(-1, 1)` = `-1` (số học) hay `0x7FFF…` (logic)?
   Không định nghĩa, và đây là lỗi im lặng (không panic). Phải tách `shr` (số học, giữ dấu) và `ushr`
   (logic), hoặc đặt tên `shr_arith`/`shr_logic`.
5. **`shl`/`shr` với số đếm ngoài [0,63]**: `shl(1, 64)` = panic? 0? Không định nghĩa; Rust panic ở
   debug, wrap ở release — đúng cái bẫy "hành vi khác nhau giữa hai bản" mà họ muốn tránh. Phải panic +
   có `checked_shl`.
6. **`>>`/`<<` không tồn tại** nên không đụng `>` (so sánh, VS:453) — ổn. Nhưng người viết C sẽ gõ
   `a >> 1` và nhận lỗi khó hiểu; thêm diagnostic "dùng `shr(a, 1)`".
7. **`bit_or` có đủ rõ?** Có — nhưng nên thống nhất hệ: `bit_and/bit_or/bit_xor/bit_not/shl/shr/ushr`
   và ghi rõ độ rộng là 64 bit có dấu (i64) để `bit_not(0) == -1`.
8. **Tách `|` trong chế độ lệnh**: closure-arg (VS:446) nuốt tới `|` **chưa đóng ngoặc**; muốn closure
   chứa `|` (or-pattern) thì đặt trong `(...)`. Phải viết thành luật, nếu không VT06 sẽ mơ hồ.

### Kết luận: **SAI ở or-pattern (1) và bảng (2); THIẾU `bit_not`, ngữ nghĩa `shr`, luật tách `|`.**

### Đề xuất sửa
- Chọn một: (a) pattern dùng `or` (`Circle(r) or Rect(w,h)`), `|` sạch tuyệt đối — đúng Q8, rẻ về lý
  thuyết, đắt về quen thuộc; (b) giữ `|` cho pattern, sửa Q8 thành "`|` chỉ dùng cho pattern và pipe,
  không bao giờ là toán tử nhị phân trong biểu thức". **Đề nghị (b)** + luật ngoặc ở mục 8, vì (a) làm
  pattern khác biệt với mọi ngôn ngữ khác mà không được lợi gì.
- Sửa bảng VS:452: `|` = pipe (lệnh) / or-pattern; `|>` = pipe (script); ghi rõ `|` không có nghĩa
  "hoặc" trong biểu thức.
- Thêm `bit_not`, `ushr` (hoặc `shr_logic`); định nghĩa `shl/shr` panic khi số đếm ≥ 64; thêm
  `checked_shl/checked_shr`.
- Thêm mục "Tách `|` trong chế độ lệnh" vào SPEC (VT02) và test AST cho `match` có or-pattern trong
  một stage.

---

## Điểm 6 — Chốt ABI ở K0, "chỉ thêm không đổi"

### Dẫn chứng
- V2:935 (D-22): "ABI lõi chốt ở K0 gồm cả object chưa cài (IOSpace, VCpu, PowerControl); sau K0 chỉ
  thêm, không đổi."
- V2:955 (T04): một task, `spec/abi.toml` đầy đủ mục 4.20+4.21 "cả object chưa cài" + `kernel.toml`,
  tiêu chí "Gửi Agent 0 duyệt (BLOCKED tới khi duyệt)". Ngân sách mặc định 3 giờ (V2:887).
- V2:408-427: bảng chỉ có **tên** object/method; không chữ ký, không kiểu tham số, không mã lỗi riêng
  cho từng method.
- V2:152: `ArchContext` chỉ có `syscall_no/arg(i)/set_ret(i,v)/ip/sp/tls` — **không** nói tham số nào ở
  thanh ghi nào, lỗi trả ở đâu (v1 có bảng đầy đủ: DS:1210-1216; v2 bỏ).
- V2:979 (giảm thiểu rủi ro): "Agent 0 duyệt `abi.toml` từng object; … **dùng thử ABI bằng kịch bản sim
  trước khi lên QEMU**" — nhưng sim là T12 (V2:963) / K1a, **sau** T04.
- V2:427: "Method chưa cài trả `NotSupported`; conformance test kiểm điều đó" — conformance test chỉ có
  ở K3 (V2:786), sau 5 mốc.

### Phân tích

**1. Đây không phải ABI, đây là mục lục.** Một ABI cần: số hiệu syscall, quy ước thanh ghi (vào/ra/lỗi),
  layout `MessageInfo`, kích thước + căn lề vùng MR, vị trí receive window trong IPC buffer, ngữ nghĩa
  `Restart` (có tới user không), mã lỗi theo từng method, và **phiên bản**. Trong V2 chỉ có số hiệu
  (0-13) và tên method. Không có chữ ký nghĩa là `vifast-gen` (T05) phải **tự phát minh** toàn bộ tham
  số — sau khi spec đã bị đóng băng và Agent 2 bị cấm sửa spec (V2:872). Đây là nghịch lý trung tâm của
  D-22: **phần được đóng băng là phần rẻ, phần bị bỏ ngỏ là phần đắt, và người bị cấm đổi là người phát
  hiện ra vấn đề.**

**2. Thiếu method sống còn, không phải "chưa cài".**
- `Tcb.SetReplyObject`: `Call` cần một Reply object (V2:229, 260) nhưng trường TCB (V2:241) không có
  `reply`, và `Configure` (V2:414) không nhận nó. Vậy Call từ một TCB chưa cấu hình reply = hành vi
  không xác định — trên **đường nóng nhất** của hệ. seL4 có đúng method này (`SetReplyObject`) vì lý do
  này.
- `Tcb.ReadState` / `Tcb.Wait`: bị bỏ cùng `TcbInfo` (V2:429), không có thay thế → điểm 1(d).
- Exit: không có syscall/method/fault kind. Phải định nghĩa (điểm 1, đề xuất 3).
- `Tcb.SetFaultEp`: chỉ tồn tại như *trường* (V2:241), không có method; `Configure` không nói.
- `Endpoint.Cancel`: không có (điểm 1(b)).

**3. T04 không khả thi trong 3 giờ, và duyệt "từng object" không bắt được lỗi.** Khối lượng thật: 14
  syscall + 15 object × trung bình 4-5 method ≈ **80 entry point**, mỗi cái cần chữ ký + mã lỗi + ngữ
  nghĩa; cộng 3 bộ quy ước gọi (riscv64/aarch64/x86_64), layout IPC buffer, 22 mã lỗi, attrs trang cho
  3 arch, tham số MCS. Một người làm đúng cần 3-5 **ngày**. Và Agent 0 duyệt một bảng tên thì không thể
  phát hiện "thiếu SetReplyObject" hay "Suspend không có mode" — loại lỗi này chỉ lộ khi viết kịch bản.

**4. "Chỉ thêm, không đổi" không cứu được, vì phần lớn thay đổi cần thiết là "đổi".** Thêm method vào
  object đã có là "thêm" (may mắn). Nhưng: thêm **trường** vào TCB/Reply; thêm **trạng thái** vào
  ThreadState; đổi **ngữ nghĩa** `Suspend`; đổi **kích thước** slot cap (F14); đổi layout `MessageInfo`
  — tất cả đều là "đổi", và tất cả đều sẽ phải làm (điểm 1, 2, 5). Tiền lệ đúng là thứ họ đang copy:
  seL4 đã **phá vỡ** ABI khi thêm MCS và Reply object (12.0) — không có "chỉ thêm" trong lịch sử seL4.

**5. Không có version, không có van sửa.** Không `abi_version`, không `min_compat`, không syscall trả
  thông tin build/ABI, không golden file. Nghĩa là: (i) không phân biệt được binary cũ/mới; (ii) mọi
  thay đổi là im lặng; (iii) "chỉ thêm" không kiểm được bằng máy.

**6. Freeze trước khi có sim là đảo ngược biện pháp giảm thiểu của chính tài liệu** (V2:979 nói dùng thử
  ABI bằng sim *trước* khi lên QEMU, nhưng sim ở T12/K1a sau T04/K0).

### Kết luận: **SAI (D-22 như viết) + THIẾU (version, chữ ký, exit/wait/reply-object). T04 nên tách 4.**

### Đề xuất sửa

**A. Thu hẹp định nghĩa "đóng băng"** — chỉ 5 thứ, mọi thứ khác ghi `stability = "reserved"`:
1. Số hiệu syscall 0-13 và quy ước thanh ghi 3 arch (kèm bảng như DS:1210-1216).
2. Layout `MessageInfo` (34 bit) + số MR trong thanh ghi theo arch + vị trí receive window.
3. 22 mã lỗi + quy ước trả lỗi (một thanh ghi `error`, một thanh ghi `value`).
4. Kích thước/căn lề object theo arch.
5. Quy tắc "ID không bao giờ tái sử dụng; xoá = `deprecated`, giữ số".

**B. Versioning**: `abi.toml` có `version = 1`, `min_compat = 1`; `DebugGetInfo -> {abi_version,
build_id, config_hash}`; `test/abi/abi-v1.golden` sinh từ `vifast-gen`, gen-check so sánh → mọi thay đổi
là diff nhìn thấy được, phải có quyết định. `v2 gate --fast` chạy gen-check này.

**C. Tách T04**:
- `T04a` lõi: Untyped/CNode/TCB/Endpoint/Notification/Reply/SchedContext + MessageInfo + errors + 3 quy
  ước gọi + slot layout. (Reviewed kỹ.)
- `T04b` object reserved: VSpace/PageTable/Frame/IrqControl/IrqHandler/IOSpace/VCpu/PowerControl — chỉ
  số + `NotSupported` + `stability="reserved"`. (Duyệt nhanh.)
- `T04c` "ABI trial": 20 kịch bản viết tay trên **model** (chưa cần kernel): cap copy/revoke, call/reply,
  suspend-while-blocked, kill-while-blocked, exit, 4-cap transfer, reply bị xoá. Mục tiêu: mỗi kịch bản
  phải viết được **chỉ bằng ABI**. Kịch bản nào không viết được = ABI thiếu.
- `T04d` checklist duyệt cho Agent 0 (bắt buộc): mỗi method có (chữ ký, mã lỗi, tiền điều kiện, hậu điều
  kiện, trạng thái TCB trước/sau). Không có đủ 5 mục thì không duyệt.

**D. Bổ sung ngay vào ABI**: `Tcb.SetReplyObject`, `Tcb.ReadState` (hoặc `Wait`), `FaultKind::Exit`,
`Tcb.Suspend(mode)`, `Endpoint.Cancel`.

---

## Điểm 7 — Lộ trình §10 và queue §13

### Dẫn chứng
- V2:776-789 (§10.1): K0 (spec) → K1a (objmem/CSpace) → K1b (TCB/sched/MCS) → K1c (IPC) → K2 (VM) →
  **K3 (boot + plat-riscv64 + init + ktest)** → K4/K5 (arch) → K6 (user space).
- V2:950-967 (§13): T01..T16. **Không có task boot/QEMU/plat nào.**
- V2:646 (§9.1): L3 QEMU "commit chạm platform/user; **cổng ngày**".
- V2:762: `v2 gate --fast` gồm "L1 10.000 seed". V2:781: tiêu chí K0 là "`v2 gate --fast` chạy".
- V2:963 (T12): SimPlatform "chưa VM" phụ thuộc T09, T10. V2:962 (T11): chỉ bảng ThreadState, **không**
  scheduler/timer.

### Phân tích

**1. Thứ tự nhất quán trên giấy, nhưng thiếu mốc chứng minh kernel chạy.** Lộ trình đi đúng hướng
"tất định trước, phần cứng sau" (nguyên tắc 8), nhưng **vertical slice đầu tiên là K3**, sau 6 dòng công
việc và sau khi đã làm xong objmem, CNode+MDB, TCB, scheduler, MCS, IPC, Reply, Notification, WaitAddr,
VSpace, ASID, shootdown. Đây đúng kiểu "big bang integration" mà lịch sử v1 đã phạt: 973 commit, 11
vòng cho một lỗi, một tệp 17.715 dòng (DS:1777-1786, 1925). Không có gì chứng minh kernel *chạy* cho tới
khi mọi thứ cùng đúng.

**2. Queue vòng 1 thiếu hẳn nhánh boot.** 16 task, không task nào chạm QEMU. Hệ quả: (i) rủi ro lớn nhất
của boot — UEFI riscv64, mà chính tài liệu đánh dấu [GT] (V2:441) — không được thử cho tới K3; (ii) §9.1
hứa "cổng ngày" có QEMU nhưng không có QEMU trước K3, tức lời hứa sai trong nhiều tháng; (iii) `v2 gate`
(V2:763) gồm L3 "các arch đã port" nên trước K3 `gate` không thể xanh — nghĩa là cổng đầy đủ không tồn
tại trong suốt giai đoạn dễ sai nhất.

**3. Tiêu chí K0 rỗng nghĩa.** `gate --fast` gồm L1 10k seed, mà L1 cần SimPlatform (T12), mà T12 cần
T09/T10. Vậy ở K0, L1 chạy 0 kịch bản và **xanh vô nghĩa**. Một cổng xanh mà không chứng minh gì là
cổng tệ hơn không có cổng — nó tạo niềm tin sai. Định nghĩa lại: K0 = gen-check + fmt + clippy + luật
cấu trúc + L0; L1 bật khi T12 xong, và ghi rõ trong STATE.

**4. T12 (SimPlatform) thiếu phụ thuộc scheduler.** Muốn chạy kịch bản nhiều hart, có preempt, có
timer/IPI (V2:699, 709) thì cần ready queue + timer + `Decision` dispatch; T11 chỉ là bảng chuyển trạng
thái. Hoặc thêm `T11b` (scheduler tối thiểu + timer ảo), hoặc thu hẹp T12 thành "harness một hart, bước
thủ công". Không chọn thì T12 sẽ phình gấp ba.

**5. K1a phụ thuộc ngầm vào T12/T13.** Tiêu chí K1a là "sim kịch bản cap/untyped 100.000 seed; khớp mô
hình" (V2:782) — tức cần T12 **và** T13, cả hai nằm sau T10. Không sai, nhưng phải vẽ ra: K1a không thể
tuyên bố xong trước T13.

**6. ViShell (sản phẩm flagship) là thứ cuối cùng.** U1 sau K6 (V2:796), VSH2 sau U1+VS2. Nghĩa là
trong phần lớn thời gian, hệ chỉ có "shell tạm (Rust)" (V2:789) — và ABI không được thử bởi ứng dụng
thật. Đề nghị: procsrv tối thiểu + shell tối thiểu (chỉ `ls/cat/echo` qua VFS) ở **K3.5**; đó là phép
thử ABI tốt nhất và rẻ nhất.

### Kết luận: **THIẾU (boot/QEMU sớm, vertical slice, scheduler cho sim) + một mâu thuẫn (cổng ngày
trước K3) + một tiêu chí rỗng (K0).**

### Đề xuất sửa
1. Thêm **K0.5** ngay sau K0: "boot riscv64 QEMU (OpenSBI nạp trực tiếp, **không** UEFI) + BootDesc +
   `DebugPutStr` + `DebugHalt` + dump trace". Tiêu chí: TAP 1 test, 50 boot 0 lỗi. Đây là mốc chứng minh
   "kernel chạy" đầu tiên, và nó gỡ [GT] UEFI khỏi đường tới hạn.
2. Thêm task **T17 boot + T18 plat-common + T19 QEMU harness** vào vòng 1 (hoặc vòng 2 ngay sau T07).
3. Đổi tiêu chí K0 (bỏ L1), ghi rõ "L1 bật từ T12".
4. Thêm **T11b**: scheduler tối thiểu (ready queue theo hart, `pick`, timer ảo) làm phụ thuộc của T12.
5. Thêm **K3.5**: procsrv tối thiểu + shell Rust tối thiểu, để ABI bị dùng thật trước K6.
6. T01 (soak 750 boot) tách thành T01a (tag + commit, 30 phút) và T01b (soak qua đêm, ngoài ngân sách
   3h) — hiện tại nó là task đầu tiên và chắc chắn vượt ngân sách.

---

## Điểm 8 — Giấy phép D-27 + SPDX + phụ thuộc

### Dẫn chứng
- V2:940 (D-27): "MIT OR Apache-2.0 cho toàn bộ mã nguồn; phụ thuộc phải tương thích".
- V2:881: "Mọi tệp mã nguồn mới có header SPDX `// SPDX-License-Identifier: MIT OR Apache-2.0`".
- V2:878: thêm crate ngoài: "thuần Rust, giấy phép tương thích MIT/Apache-2.0, ghi lý do trong JOURNAL,
  kiểm `deps.toml`" — kiểm bằng **tay**, bởi chính agent.
- V2:953 (T02): LICENSE-MIT + LICENSE-APACHE. V2:954 (T03): cổng kiểm "header SPDX".
- V2:535 (§6.7): smoltcp [GT: kiểm giấy phép].
- Hiện trạng repo: `Cargo.toml:50` đã có `license = "MIT OR Apache-2.0"`; 24/25 crate dùng
  `license.workspace = true`; **`crates/vifast-maint/Cargo.toml` thiếu**; **0/39 tệp `.rs` có header
  SPDX**; `LICENSE-MIT` có dòng copyright.

### Phân tích

**1. Biểu thức SPDX: hợp lệ và đúng thực tiễn Rust.** `MIT OR Apache-2.0` là biểu thức SPDX hợp lệ
(toán tử `OR`), và đúng là thứ Rust dùng (`license = "MIT OR Apache-2.0"` trong Cargo.toml của rust-lang).
Không có gì phải sửa. Header `// SPDX-License-Identifier: …` cũng là cú pháp chuẩn. **Kết luận: ổn.**

**2. Nhưng header theo từng tệp không phải thứ làm giấy phép có hiệu lực, và nó *đắt* hơn tưởng.**
Rust **không** đặt header ở từng tệp; nó dựa vào `Cargo.toml` + `LICENSE-*` ở gốc. Việc bắt buộc header
là mô hình REUSE (FSFE) — hợp lệ, nhưng cần thêm: `SPDX-FileCopyrightText` (nếu không, header không
xác lập dòng copyright mà MIT yêu cầu "in all copies"), và một công cụ (`reuse lint`) hoặc cổng tự
viết. Với 39 tệp hiện có 0 header, T03 sẽ hoặc fail toàn bộ, hoặc phải viết ngoại lệ — và nếu áp lên
các crate **chép từ v1** (`vifast-pte`, `vifast-elf`, `vifast-acpi`, `vifast-fs`, `vifast-virtio`,
V2:64) thì đó là sửa hàng loạt code không phải của mình, làm nhiễu blame/diff.

**3. Tệp không có comment — cổng sẽ vỡ hoặc bị lách.** Không thể có header trong: `.json`
(`R<n>.json`, `mutants.toml` thì được), `*.wasm`, `Cargo.lock`/`vis.lock` (cargo ghi lại → mất header),
`*.scn` (định dạng dữ liệu, chưa nói có comment), `*.vbc` (bytecode cache). Luật phải là: *bắt buộc
header cho đuôi có cú pháp comment; các đuôi khác dùng sidecar `.license` (kiểu REUSE) hoặc miễn trừ có
danh sách; tệp sinh bởi `vifast-gen` phải được generator phát header (hoặc miễn trừ vì sinh lại được)*.
Không có luật này, T03 sẽ có "case âm" cho một luật không thể thi hành.

**4. Ai kiểm phụ thuộc: hiện tại không ai.** V2:878 là chỉ dẫn cho agent tự khai. Một LLM tự đọc
Cargo.toml của crate mới rồi tự nói "tương thích" là **tự chứng nhận**. Cần công cụ:
`cargo-deny` (`[licenses] allow = [...]`, `confidence-threshold = 0.9`, `[bans] multiple-versions`,
`[sources] unknown-registry = "deny"`) + `cargo-about` sinh `THIRD-PARTY.md`. Hai công cụ này là
**dev-dependency**, không vào ảnh hệ thống, nên không vi phạm D-24 ("chỉ crate thuần Rust trong ảnh").
Nhưng chúng là phụ thuộc *mới* của quy trình — phải ghi vào `deps.toml` (loại `tool`), và cân nhắc:
`cargo-deny` cần DB advisory (mạng) → chốt: chỉ chạy `licenses`/`bans` offline trong `gate --fast`,
`advisories` trong nightly.

**5. Bẫy allowlist thực tế.** "Chỉ MIT/Apache" sẽ chặn hai thứ gần như chắc chắn cần:
- `unicode-ident` (và họ `unicode-*`) là `(MIT OR Apache-2.0) AND Unicode-3.0` — bắt buộc cho định danh
  UAX #31 + NFC (VS:97, VT04). Thiếu `Unicode-3.0` trong allowlist là build đỏ ngay VT04.
- `smoltcp` là `0BSD` (V2:535 đang [GT]) — tương thích, nhưng không phải "MIT/Apache".
Allowlist đề nghị: `MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, 0BSD, Unicode-3.0, Zlib,
Unicode-DFS-2016`; quyết định riêng cho MPL-2.0 (copyleft theo tệp — nên cấm để giữ tính "dễ dùng lại").

**6. NOTICE / THIRD-PARTY.** Apache-2.0 §4(d) chỉ bắt buộc NOTICE nếu bản phân phối **có** NOTICE →
đề nghị **không** tạo NOTICE (để khỏi phải lan truyền), nhưng: (i) phải có `LICENSE-MIT` +
`LICENSE-APACHE` **trong ảnh** phân phối (một ảnh QEMU là phân phối; Apache §4 yêu cầu gửi kèm bản
License, MIT yêu cầu copyright notice "in all copies"); (ii) `THIRD-PARTY.md` sinh tự động. Cụ thể:
nhúng vào ảnh và mở qua `/licenses` hoặc lệnh `licenses`.

### Kết luận: **ỔN về biểu thức giấy phép; THIẾU cơ chế kiểm (công cụ + allowlist), luật cho tệp không
comment, license field theo crate, và NOTICE/THIRD-PARTY trong ảnh.**

### Đề xuất sửa
1. Giữ `MIT OR Apache-2.0`; thêm `license.workspace = true` cho **mọi** crate (sửa
   `vifast-maint` ngay), và T03 kiểm bằng `cargo metadata` (license rỗng = đỏ), không chỉ header.
2. Luật header: danh sách đuôi bắt buộc + sidecar `.license` cho đuôi không comment + miễn trừ tệp sinh.
3. Thêm `cargo-deny` + `cargo-about` vào `deps.toml` loại `tool`; `THIRD-PARTY.md` sinh trong
   `gate --fast`; allowlist như trên (đặc biệt `Unicode-3.0`, `0BSD`).
4. Nhúng `LICENSE-MIT`, `LICENSE-APACHE`, `THIRD-PARTY.md` vào ảnh; lệnh `licenses` trong user space.
5. Không tạo NOTICE (trừ khi có lý do); nếu tạo thì phải lan truyền.

---

## Điểm 9 — Rủi ro tổng: chỗ nào chắc chắn trượt

Bối cảnh: một agent, 3 giờ/task (V2:887), 16 task vòng 1 (V2:950-967), mục tiêu 90 ngày cho K0-K2
(V2:834). Đối chiếu lịch sử: v1 có 973 commit, 17.715 dòng trong **một** tệp kernel, 11 vòng cho một
lỗi (DS:1777-1786); user space v1 (root 4.487 + client 2.169 + server 1.235 dòng, DS:1011-1050) làm
được ramfs + fd + console **trong kernel** — tức v2 K6 yêu cầu nhiều hơn thế, bằng user space, với
journal + pager.

### 9.1 Bốn chỗ chắc chắn trượt

**(1) MCS đầy đủ ở K1b.** `SchedContext` với refill queue, passive server, "SC đi theo IPC", timeout
fault (V2:245-251), cộng invariant "không TCB nào chạy khi hết budget" kiểm trong sim (V2:251), cộng
tương tác với `Reply` (V2:260) và `ReplyRecv`. Đây là phần seL4 mất **nhiều năm** để thiết kế đúng, và
nó là **trực giao** với tính đúng của capability/IPC. Với một agent 3h/task: gần như chắc chắn không
xong ở K1b, và vì K1c/K2 phụ thuộc K1b, nó **chặn cả dòng lõi**. Tệ hơn: MCS sai không làm test đỏ —
nó làm budget lệch, và phải đo mới thấy.

**(2) objmem + `get2_mut` + Miri.** Hai vấn đề độc lập:
- *Số object*: IPC cần đồng thời sender TCB, receiver TCB, endpoint, reply, IPC buffer, và 2 slot CNode
  (MDB) — **≥4-7 object**. `get2_mut` (V2:174) không đủ; sẽ phải thêm `get3_mut/get4_mut` hoặc đổi sang
  API closure. Đây là **đổi ABI nội bộ** sau freeze.
- *Aliasing*: `place<T>` ghi `T` vào `Vec<u8>`/physmap rồi `get_mut` trả `&mut T`. Dưới Stacked
  Borrows/Miri, việc tạo `&mut` trỏ vào bộ nhớ mà store còn giữ `&[u8]`/`&mut [u8]` là UB. Muốn Miri
  xanh phải dùng raw pointer + `UnsafeCell` + không bao giờ form reference tới cả buffer, và **không**
  trả `&mut` từ `&self`. Đây là thiết kế lại crate ở K1a — đúng lúc ABI vừa đóng băng.
  (Điều này cũng đúng với mọi object store kiểu này; seL4 tránh bằng C.)

**(3) 3 arch.** Mỗi arch: boot (UEFI/OpenSBI/PSCI), trap, syscall, timer, IPI, IRQ (PLIC/AIA vs GICv3+ITS
vs x2APIC+IOAPIC+MSI), SMP khởi động hart phụ, page table, ASID/PCID, ảo hoá — trong **2.500 dòng**
(V2:471). riscv64 UEFI là [GT] (V2:441); x86 có KPTI + `-cpu max`; aarch64 EL2. K4/K5 kiểu gì cũng trượt
mốc; và mỗi arch là nguồn flake mới (v1 đã có: x86 shootdown AP→BSP, DS:1725-1733).

**(4) Cổng L1 10.000 seed < 60 s** với 10 invariant mỗi event gồm mô hình TLB + kiểm PerHart + budget
(V2:716-727, 644). Không đạt được với sim chạy page-table thật + object thật. Hệ quả không phải "chậm"
mà là **cổng bị nới** — vi phạm D-11/D-15, đúng thứ v1 đã làm và đã trả giá. Phải đo từ ngày đầu và
đặt hai mức (1k fast / 100k nightly) ngay ở K1a, trước khi viết kịch bản.

### 9.2 Ba chỗ rất có thể trượt (nêu để theo dõi)
- `vifast-core < 15.000 dòng` + tệp ≤ 1.500 dòng + cấm `cfg(target_arch)` (V2:614-619): với MCS + IPC +
  VM + ASID + shootdown + IRQ + preempt + trace, 15k dòng là lạc quan; luật 1.500 dòng sẽ sinh các tệp
  cắt cơ học, làm review khó hơn — đúng lỗi v1 (17.715 dòng) mà họ đang muốn tránh.
- ViScript VS0→VS2 (VS:817-820) lớn hơn cả kernel: lexer 2 chế độ + typechecker hai chiều + trait/generic
  + interpreter + bytecode + VM + xác minh + RC/CoW + async có cấu trúc + LSP + 4 host. Đây là dòng rủi
  ro cao nhất vì là *ngôn ngữ mới* (không biết cái gì chưa biết), và nó không được hưởng lợi từ sim.
- Hai tài liệu ~2.000 dòng với ≥15 mâu thuẫn nội bộ (báo cáo này) → Agent 2 sẽ BLOCKED hoặc tự phát
  minh. Với 3h/task, "tự phát minh" là mặc định — và đó là cách ABI chết.

### 9.3 Ba việc nên cắt hoặc đảo thứ tự

**Cắt 1 — cơ chế MCS (giữ object + API).** K1b dùng "MCS-lite": một `budget/period` mỗi TCB, hết budget
thì round-robin, **không** refill queue, **không** passive server, **không** timeout fault. `SchedContext`
vẫn có trong ABI (đã freeze), `Configure` nhận tham số nhưng `NotSupported` cho phần chưa cài. Làm MCS
đầy đủ ở **K7**, sau khi user space chạy. Lý do: nó là phần khó nhất, không cần cho tính đúng của
capability/IPC, và đang chặn 6 task.

**Cắt 2 — 2/3 arch + UEFI riscv64.** Chỉ riscv64 QEMU, **OpenSBI nạp trực tiếp** (bỏ UEFI, bỏ [GT]
V2:441) cho tới K6. aarch64 thành K6.5, x86 thành K7.5 (hoặc bỏ hẳn khỏi "90 ngày"). Lý do: mỗi arch là
~2.500 dòng + asm + 3 bộ điều khiển ngắt + một nguồn flake; và "chạy 3 arch" không phải điều kiện để
chứng minh *thiết kế* đúng — sim + 1 arch là đủ.

**Đảo 3 — thứ tự: boot + sim trước objmem/CSpace, và thu hẹp đóng băng ABI.**
`K0 → K0.5 (boot QEMU + DebugHalt) → T12 sim (object store giả `Vec<Box<dyn Any>>`, chưa cần objmem)
→ T10 CNode/MDB (test-first trên sim) → T07 objmem (thay store, Miri) → …`
Lý do: (i) mốc "kernel chạy" đến trong tuần đầu thay vì sau 6 dòng; (ii) CNode/MDB/Revoke được phát
triển test-first, thay vì viết xong mới có invariant checker; (iii) objmem — chỗ dễ UB nhất — được thay
*vào* một harness đã chạy, nên sai Miri lộ ngay; (iv) và đóng băng ABI chỉ 5 thứ (điểm 6.A) + version.
Kèm theo: T04 tách 4 (điểm 6.C), và bỏ `IOSpace/VCpu/PowerControl/MSI/KPTI/SetTls` khỏi "frozen" —
thay bằng dải số reserved.

---

## Phát hiện ngoài 9 điểm (vẫn là lỗi thiết kế, không phải chính tả)

**N1. Push migration vi phạm chính luật nó đặt ra.** V2:280: đẩy TCB sang hart Y "đang Idle", nhưng
V2:280 lại từ chối work-stealing vì "phải đọc trạng thái hart khác", và V2:353 (luật 3) cấm chạm
`PerHart` của hart khác. Để biết Y idle phải đọc trạng thái Y — đúng thứ vừa bị cấm. Sửa: mỗi hart ghi
cờ `idle` của mình vào một mảng cache-line-aligned (ghi bởi chủ, đọc atomic bởi người khác) — luật 3 nói
"không **sửa**", nên chỉ cần nói rõ "đọc cờ công khai là ngoại lệ được phép". Hoặc idle hart gửi
`Pull` (nhưng họ từ chối kéo).

**N2. Hàng chờ WaitAddr không có chỗ chứa.** V2:272: "Hàng chờ nằm trong **header Frame**". `ObjHeader`
là 24 byte cố định `{kind, state, pad, refs, id}` (V2:168) — không chứa được danh sách. Nếu "header" là
24 byte đầu của **trang** Frame thì user ghi được (Frame là của user) ⇒ user sửa được con trỏ danh sách
kernel ⇒ lỗi bảo mật. Sửa: bảng băm phía kernel khoá theo `(frame_id, offset)` với số bucket =
`f(MAX_TCB)`; hoặc mỗi TCB giữ node và list per-bucket. Đồng thời phải ghi rõ: đọc giá trị atomic +
barrier trước khi block + kiểm lại dưới lock, nếu không "không mất đánh thức" (V2:273) là tuyên bố suông.

**N3. `str` index.** EBNF cho `postfix '[' expr ']'` trên mọi giá trị (VS:932); `str` là UTF-8
(VS:145); không có luật byte/char/grapheme. Với tiếng Việt/Nhật, `s[i]` panic giữa ký tự. Cấm `str[i]`
(lỗi kiểu), thêm `.chars()/.graphemes()/.byte_at(i)?`. (Đã nêu ở điểm 2.)

**N4. Consent theo hash sai đối tượng.** VS:549: nhớ quyền "theo hash tệp". Nhưng chương trình `app.vis`
`use ./util` — thay `util.vis` là hành vi đổi mà hash tệp chính không đổi ⇒ chạy với quyền đã cấp.
Sửa: hash **closure** (nguồn + toàn bộ phụ thuộc + phiên bản compiler) — đúng bằng hash cache bytecode
ở VS:682.

**N5. Từ khoá/EBNF thiếu.** `const` không có trong danh sách từ khoá (VS:100) nhưng `const_decl` có
trong EBNF (VS:908); `assign`, `lambda`, `field_updates`, `types` được dùng mà không định nghĩa;
`postfix '[' types ']'` (VS:932) nhập nhằng với index: `json.parse[list[User]](text)` có thể parse thành
`parse[list[User]]` (index) trước khi thử generic. Phải có luật (chỉ thử `[types]` khi theo sau là `(`,
hoặc dùng cú pháp turbofish riêng).

**N6. VT02 không chặn VT04-VT06.** VS:844: VT02 "Gửi Agent 0 duyệt; **không chặn** VT03 tới VT09". Cùng
lúc VD-08 (VS:884) nói thay đổi SPEC chỉ qua Agent 0. Nghĩa là lexer/parser được xây trên spec **chưa
duyệt** → sửa lại từ đầu. Sửa: VT02 chặn VT04/VT05/VT06; hoặc VT04/VT05 ghi rõ là "spike, sẽ vứt".

**N7. `ObjHeader` có hai cơ chế vòng đời.** `refs: u32` (V2:168) gợi ý đếm tham chiếu, nhưng luật
finalise là "Revoke/Delete → finalise" (V2:187, 191-206). Hai cơ chế chồng nhau: `refs` dùng để làm gì,
khi nào về 0, ai giảm? Không định nghĩa. Và `state: u8` trong header vs `ThreadState` của TCB — cái nào
là thật? Sửa: bỏ `refs` (vì vòng đời do cap + finalise quyết định) hoặc định nghĩa nó là "số cap trỏ
tới" và finalise khi 0.

**N8. Slot cap 32 byte không đủ.** V2:213: `{objref, kind, rights, badge, mdb_prev, mdb_next,
mdb_depth}`. `objref` = pa:u64 + id:u64 = 16; badge u64 = 8 ⇒ 24; `mdb_prev/next` phải chỉ được tới
2^20 slot (V2:212) ⇒ u32 mỗi cái = 8 ⇒ **32**, còn kind+rights+depth = 3 ⇒ ≥40 byte (căn 8). Với 2^20
slot: 42 MB thay vì 33 MB — không chết, nhưng con số [ƯL] đang sai 25% và sẽ vỡ ngân sách bộ nhớ nếu
ai đó tin nó. Sửa: hoặc 40 byte, hoặc nén (id 32 bit + generation, mdb link 16 bit với CNode ≤ 2^16).

**N9. Notification đổi ngữ nghĩa không ghi lý do.** V2:266: "đánh thức **một** waiter, giao word, xoá
word"; v1: "đánh thức **mọi** waiter" (DS:586). Với IRQ → notification → nhiều driver, "một waiter +
xoá word" làm mất sự kiện cho người còn lại. Nếu đó là chủ ý (semaphore nhị phân), phải ghi rõ và phải
nói broadcast nằm ở user space *như thế nào* (một endpoint + N notification? một `NotifyAll` RPC?).

---

## Phụ lục A — ABI tối thiểu cần bổ sung trước K0 (bảng để Agent 0 duyệt)

| Hạng mục | Nội dung | Lý do |
|---|---|---|
| Trạng thái | `suspended: bool`, `restart: bool` trực giao với `ThreadState`; bảng chuyển tiếp cho `stop/resume` | điểm 1(a) |
| TCB | `SetReplyObject`, `ReadState`, `SetFaultEp`, `Suspend(mode)` | điểm 1(b,d), 6 |
| Fault | `FaultKind::Exit{code}` | exit không cần API mới |
| Endpoint/Reply | `Cancel` (hoặc định nghĩa "finalise Reply ⇒ caller nhận `Cancelled`" là đủ cho mọi đường) | điểm 1(b) |
| Lỗi | `BrokenPipe`, `Stopped` | điểm 1(f) |
| Quy ước gọi | bảng 3 arch: số syscall, tham số, `error`, `value`, `label`, MR | điểm 6 |
| Version | `abi_version`, `min_compat`, `DebugGetInfo`, golden file | điểm 6 |
| Terminal | `term.vidl`: termios, fg group, resize | điểm 1(e) |
| Ổ đĩa | không có gì | — |

## Phụ lục B — Checklist duyệt ABI cho Agent 0 (mỗi method 5 mục)

1. Chữ ký đầy đủ (kiểu từng tham số, kiểu trả về, số MR tiêu thụ).
2. Mã lỗi có thể trả (tập con của 22 mã + 2 mã mới).
3. Tiền điều kiện (quyền cap, trạng thái object, trạng thái TCB).
4. Hậu điều kiện (trạng thái sau, ai được đánh thức, message nào được giao).
5. Kịch bản sim viết được **chỉ bằng ABI** (T04c) — nếu không viết được, ABI thiếu.

## Phụ lục C — Thứ tự đọc cho Agent 2 (chống trôi)

K0: đọc §0, §1, §4.1-4.6, §4.20-4.22, §8, §11, §12 của V2 + phụ lục A của VS. **Không** đọc §6-§7
V2 (user space chưa tới lượt). Khi gặp mâu thuẫn trong báo cáo này: ghi BLOCKED theo V2:889, **không**
tự chọn — vì mỗi lựa chọn tự phát sẽ thành ABI ngầm.

---

*Hết báo cáo. 15 phát hiện (4 P0, 6 P1, 5 P2), 9/9 điểm có kết luận, 3 việc cắt/đảo, 2 phụ lục thi
hành. Không có phát hiện nào dựa trên suy đoán về mã chưa tồn tại: mọi dẫn chứng là câu trong hai tài
liệu hoặc số đo trong hồ sơ v1.*
