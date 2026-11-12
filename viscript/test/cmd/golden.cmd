// SPDX-License-Identifier: MIT OR Apache-2.0
// 33 GOLDEN CmdCST — ST-06 (22 ca pha A + 1 ca `.vish` nhiều dòng + 10 ca bổ sung sau review vòng 1)
// (D2-05: đỏ trước, pha B mới hiện thực `parse(Mode::Command)`).
//
// Nguồn: `docs/viscript/ViScript-thiet-ke.md` §9.3 (phân tích dòng lệnh), §9.4 (bảng xung đột 12
// hàng), §9.5, §A.7 (văn phạm CmdCST); `docs/viscript/SPEC-lex.md` §5–§6 (token chế độ lệnh);
// VD-21/VD-22 (hai pha: `vis-syntax` sinh CmdCST **chưa định kiểu**, `cmd_lower` ở vòng 2 quyết
// kiểu chữ ký).
//
// ĐỊNH DẠNG mỗi ca (một dòng):
//   <nguồn> => <sexp>          CmdCST, dạng ngoặc hoá
//   <nguồn> => !ERR            parse phải hỏng, **đúng 1** lỗi (không ở token đầu)
//   <nguồn> => !ERR "<chuỗi>"  như trên, và thông điệp lỗi phải chứa <chuỗi>
// Dấu phân cách: lần xuất hiện **cuối cùng** của `" => "` mà phần sau bắt đầu bằng `(` hoặc `!`
// (vì nguồn có thể chứa `=>` của closure). Nguồn **nhiều dòng** viết `\n` (hai ký tự) — harness
// đổi thành xuống dòng thật trước khi parse.
//
// QUY ƯỚC SEXP CmdCST (áp cho cả 33 ca; dùng cho harness `crates/vis-syntax/tests/cmd.rs`):
// - Literal/tên giữ **nguyên văn nguồn** (NFC): `(word foo)`, `(flag -l)`, `(str "hi")`.
// - `pipeline := stage ('|' stage)*` ⇒ `(pipe S1 S2 …)` — danh sách phẳng, **giữ thứ tự** viết.
// - `&` nền (chỉ token cuối, có khoảng trắng trước) ⇒ `(bg (pipe …))`.
// - `;` tách pipeline (§9.3/§9.4) ⇒ `(items P1 P2 …)`.
// - `stage := HEAD ARG*` ⇒ `(stage HEAD A1 A2 …)`; HEAD:
//     NAME                    ⇒ `(cmd ten)`
//     '^' NAME                ⇒ `(ext ten)`
//     '$' IDENT ('.' IDENT)*  ⇒ `(var x)` / `(var a b)`; riêng `$env.NAME` ⇒ `(env NAME)`
//     '(' expr ')'            ⇒ `(group EXPR)`
// - arg:
//     WORD                    ⇒ `(word ng-văn)`  (glob `*`, `**`, char-class `[ab]` nằm trong word)
//     `--x` / `-x` / `--`     ⇒ `(flag --x)`; có `= expr` ⇒ `(flag --x EXPR)`
//     STRING                  ⇒ `(str <nguyên-văn-token, kể cả dấu nháy>)`
//     '$' IDENT ('.' IDENT)*  ⇒ `(var x)` / `(env NAME)`
//     '(' expr ')'            ⇒ `(group EXPR)`
//     IDENT '=>' body         ⇒ `(closure x EXPR)`
//     field_shorthand         ⇒ `(shorthand EXPR)`
// - EXPR dùng **đúng** quy ước sexp biểu thức của ST-05/VD-34 (`(field X ten)`, `(% a 2)`, `(+ 1 2)`,
//   `(list 1 2)`…), gồm postfix `.`/gọi/index, nên `x.size` ⇒ `(field x size)`.
// - Không sinh node cho khoảng trắng/chú thích; chú thích chỉ là trivia (CST vẫn lossless).
// - Kết hợp trái lồng trái (như `test/expr/golden.expr`); pipeline là danh sách nên **không** lồng.
// - **Gốc**: tệp **một dòng** ⇒ sexp của dòng đó (tiện cho một lệnh); tệp **nhiều dòng** ⇒
//   `(shell ITEM…)`; `shell_let` ⇒ `(let ten PIPE)` (có kiểu ⇒ `(let ten TYPE PIPE)`).
// - `[ab]`: lexer cắt `[`, `ab`, `]`; parser **ghép** khi span **liền kề** ⇒ `(word [ab])` (VD-40 #3);
//   có khoảng trắng bên trong = lỗi.
// - `-5` giữ **từ trần** `(word -5)`; `-l` / `--x` / `--` là `(flag …)` (VD-40 #2/#4).
// - `>` `<` `>=` `<=` `==` `!=` ngoài `( … )` là **từ chưa định kiểu** `(word >)` — `cmd_lower` quyết
//   nghĩa (so sánh/…); **không** có redirect, ghi tệp bằng `| save`. (Khối `vish-session` của tài
//   liệu dùng `where size > 5mb` nên CmdCST phải giữ `>` được — xem JOURNAL ST-06.)
//
// 12 ca đầu = **mỗi hàng bảng §9.4, đúng thứ tự** (#1..#12); #13..#22 là **ca mơ hồ** (comment nói
// rõ: mơ hồ ở đâu · thiết kế nói gì · chọn cách nào và vì sao); #23 là `.vish` nhiều dòng; #24..#33 là
// 10 ca bổ sung sau review vòng 1 (let có kiểu, closure `|>`, `&;`, vùng từ liền kề, `flag=expr`, nhóm-stage).

// #1  |  — pipe ở độ sâu 0 (không phải or-pattern)
ls | grep foo => (pipe (stage (cmd ls)) (stage (cmd grep) (word foo)))

// #2  |>  — chế độ lệnh **không** có `|>` (chỉ pipe biểu thức) ⇒ lỗi, không phải pipe
ls |> grep foo => !ERR

// #3  > <  — so sánh trong tham số biểu thức `( … )`, **không** redirect
ls | (a > b) => (pipe (stage (cmd ls)) (stage (group (> a b))))

// #4  #  — chú thích khi **đứng đầu từ**; phần còn lại của dòng bị bỏ qua
ls # ghi chú => (pipe (stage (cmd ls)))

// #5  %  — modulo; jobspec dùng `job 1` chứ không phải `%1`
ls | (a % 2) => (pipe (stage (cmd ls)) (stage (group (% a 2))))

// #6  * **  — glob (một cấp/đệ quy) nằm trong từ trần
ls *.txt **/*.md => (pipe (stage (cmd ls) (word *.txt) (word **/*.md)))

// #7  -x --x --  — cờ; `--` kết thúc cờ (phần sau là từ trần)
ls -l -- x => (pipe (stage (cmd ls) (flag -l) (flag --) (word x)))

// #8  $  — biến phiên `$x` và biến môi trường `$env.NAME`
echo $x $env.HOME => (pipe (stage (cmd echo) (var x) (env HOME)))

// #9  [ (  — `[ab]` là glob char-class (giữ trong từ); biểu thức/list literal phải trong `( )`
ls [ab] | (1 + 2) => (pipe (stage (cmd ls) (word [ab])) (stage (group (+ 1 2))))

// #10 ; &  — `;` tách pipeline, `&` nền ở cuối (chỉ khi là token cuối, có khoảng trắng trước)
ls; pwd & => (items (pipe (stage (cmd ls))) (bg (pipe (stage (cmd pwd)))))

// #11 ^ten  — ép gọi chương trình ngoài (chỉ ở **đầu stage**)
^git status => (pipe (stage (ext git) (word status)))

// #12 Ghi tệp — bằng `| save out.txt`, **không** dùng `>`
ls | save out.txt => (pipe (stage (cmd ls)) (stage (cmd save) (word out.txt)))

// #13 Mơ hồ: `>` là redirect (quen shell) hay so sánh? · Thiết kế: §9.4 hàng 3 + §6 —
//     "`>` `<` là so sánh, **không** redirect; ghi tệp bằng `| save`" · Chọn: CmdCST **chưa định
//     kiểu** nên `>` là từ trần `(word >)`; `cmd_lower` quyết so sánh; **không** redirect. (Khối
//     `vish-session` `where size > 5mb` buộc `>` ngoài ngoặc phải parse được — xem JOURNAL.)
echo hi > out.txt => (pipe (stage (cmd echo) (word hi) (word >) (word out.txt)))

// #14 Mơ hồ: `#` mở chú thích hay là ký tự trong URL? · Thiết kế: §9.4 hàng 4 + §6 — "`#` khi
//     **đứng đầu từ**; `https://x#y` giữ nguyên" · Chọn: `#` giữa từ là ký tự thường ⇒ một WORD.
curl https://x#y => (pipe (stage (cmd curl) (word https://x#y)))

// #15 Mơ hồ: `2*3` là glob hay phép nhân? · Thiết kế: §9.4 hàng 6 (`*` glob ở chế độ lệnh; nhân
//     chỉ trong biểu thức) · Chọn: ngoài `( … )` là **glob trong word**; muốn nhân phải `(2 * 3)`.
ls 2*3 => (pipe (stage (cmd ls) (word 2*3)))

// #16 Mơ hồ: `-5` là số âm hay cờ? · Thiết kế: §9.3/§9.4 hàng 7 — "`-5` là số nếu tham số kiểu
//     số, ngược lại là cờ"; A.7 `flag := '-' IDENT…` **không** cho chữ số · Chọn (VD-40 #2): parser
//     giữ **từ trần** `(word -5)`; `cmd_lower` (vòng 2, VD-22) phân biệt số/cờ.
ls -5 => (pipe (stage (cmd ls) (word -5)))

// #17 Mơ hồ: `[ab]` là glob char-class hay list literal? · Thiết kế: §9.4 hàng 9 — "list literal
//     phải trong `( )`; glob char-class giữ nguyên" · Chọn: `[ab]` trần là **word glob**; list
//     literal phải viết `ls ([1, 2])` (group + list).
ls [ab] => (pipe (stage (cmd ls) (word [ab])))

// #18 Mơ hồ: `|` trong ngoặc là pipe hay or-pattern? · Thiết kế: §9.4 hàng 1 + §A.7 ghi chú 5 —
//     `|` chỉ là pipe ở **độ sâu 0**; biểu thức **không** có `|` · Chọn: **lỗi** tại `|` trong
//     ngoặc (1 lỗi, không ở token đầu).
ls | (a | b) => !ERR

// #19 Mơ hồ: closure-arg kéo tới đâu? · Thiết kế: §A.7 — `closure_body` kéo tới `|`/`;`/`&`/NEWLINE
//     ở **độ sâu 0**; ở đây hết dòng · Chọn: thân closure là `x.size > 1`; `>` trong thân là so
//     sánh (biểu thức), không phải redirect.
ls | filter x => x.size > 1 => (pipe (stage (cmd ls)) (stage (cmd filter) (closure x (> (field x size) 1))))

// #20 Mơ hồ: `$env` là biến phiên tên `env` hay tiền tố môi trường? · Thiết kế: §9.3/§9.4 hàng 8 —
//     `$env.NAME` là môi trường; `$` IDENT ('.' IDENT)* là biến · Chọn: `$env` **không** có `.NAME`
//     ⇒ biến phiên tên `env`.
echo $env => (pipe (stage (cmd echo) (var env)))

// #21 Mơ hồ: `&` giữa dòng là nền hay phân tách? · Thiết kế: §9.3/§6 — "`&` nền chỉ khi là token
//     **cuối** và có khoảng trắng trước" · Chọn: `&` giữa dòng ⇒ **lỗi** (1 lỗi, không ở token đầu).
ls & pwd => !ERR

// #22 Mơ hồ: `^` giữa dòng ép gọi ngoài hay là ký tự trong từ? · Thiết kế: §5/§6 ("`^` **chỉ** ở
//     đầu stage"; lexer ST-04 báo `VS0110` nếu không) · Chọn: `^` giữa dòng ⇒ **lỗi từ vựng**
//     `VS0110` (1 lỗi, không ở token đầu) — lexer không đổi (ST-04 DONE).
ls ^x => !ERR

// #23 `.vish` — tệp **nhiều dòng**: mỗi dòng một item, gốc `(shell …)`; `shell_let` ⇒ `(let …)`
let out = ls | grep foo\npwd => (shell (let out (pipe (stage (cmd ls)) (stage (cmd grep) (word foo)))) (pipe (stage (cmd pwd))))

// #24 review vòng 1: `let` **có kiểu** (A.7 `shell_let := 'let' IDENT [':' type] '=' pipeline`) —
//     `:` và `=` không phải metachar nên dính trong WORD; parser gom type tới WORD `=`.
let x: int = ls | grep foo => (let x int (pipe (stage (cmd ls)) (stage (cmd grep) (word foo))))

// #25 review vòng 1: `let` có kiểu nhưng **thiếu `=`** ⇒ lỗi (A.7); 1 lỗi, không ở token đầu.
let x: int => !ERR

// #26 review vòng 1: thân closure **không** dừng ở `|>` (A.7: chỉ `|`/`;`/`&`/NEWLINE ở độ sâu 0);
//     `|>` trong thân là pipe **biểu thức**.
ls | filter x => a |> f => (pipe (stage (cmd ls)) (stage (cmd filter) (closure x (|> a f))))

// #27 review vòng 1: `&` chỉ hợp lệ ở **cuối dòng** (VD-40 #1) ⇒ `&;` là lỗi.
ls &; pwd => !ERR

// #28 review vòng 1: vùng từ gộp theo span **liền kề** ⇒ `foo[ab]` là **một** từ.
ls foo[ab] => (pipe (stage (cmd ls) (word foo[ab])))

// #29 review vòng 1: `[ab]x` (glob rồi WORD liền kề) là **một** từ.
ls [ab]x => (pipe (stage (cmd ls) (word [ab]x)))

// #30 review vòng 1: `[ab][cd]` (hai cụm liền kề) là **một** từ.
ls [ab][cd] => (pipe (stage (cmd ls) (word [ab][cd])))

// #31 review vòng 1: `--x=1` ⇒ `(flag --x 1)` (A.7 `flag := '--' IDENT ['=' expr]`), vế phải là biểu thức.
ls --x=1 => (pipe (stage (cmd ls) (flag --x 1)))

// #32 review vòng 1: `-l=x` ⇒ `(flag -l x)`.
ls -l=x => (pipe (stage (cmd ls) (flag -l x)))

// #33 review vòng 1: nhóm `( … )` là **cả stage** (A.7) ⇒ không được có tham số sau nhóm.
ls | (a) b => !ERR
