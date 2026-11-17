# SO-NO.md — Sổ nợ duy nhất của dòng ViScript

**Task**: ST-145 (lập sổ) · **ST-148** (cập nhật số 142/143 theo **VD-149**) · **ST-149** (đóng **#136**) ·
**ST-153** (ghi chú: ca 7 self-test của ST-147 **đóng ở ST-153**, §7.4 — không cấp số mới) ·
**ST-151** (đóng **#135**) ·
**ST-154** (đóng **#144**; `#143` → **chuyển VS3** theo **VD-154(b)**; thêm **#145** theo **VD-154(c)**) ·
**ST-157** (đóng **#110** — VM đưa `argv` vào tham số **không-`cap`** của `main`, parity interp↔VM theo **VD-156**) ·
**ST-158** (đóng **#129** — danh sách 7 khối tài liệu bị loại trừ (VD-36) về **một nguồn** `test/doc_excluded.rs`;
4 tệp test dùng `include!`, không chép số) ·
**Luật gốc**: `DECISIONS.md` **VD-147(3)** (kèm VD-142, VD-143, VD-145, VD-148, VD-149, VD-153, VD-154, VD-156)
**Ngày lập**: 2026-11-16 · **Người ghi**: Agent 2 dòng ViScript · **HEAD khi lập**: `b1ee0b33`
**Cập nhật**: 2026-11-17 (**ST-160** — dán đợt 29, mở **#146** (VD-158), sổ **147 mục = 90 mở / 34 đóng / 21 VS3 / 1 không làm / 1 điều hướng**) · **HEAD khi cập nhật**: `567350e1`
**Tệp duy nhất**: `viscript/agent/SO-NO.md` — từ R25, mọi báo cáo trích nợ **bằng số trong sổ này**.

## 0. Mục đích

Sổ này là **nguồn duy nhất** về nợ kỹ thuật của dòng ViScript kể từ đợt 25 (VD-147(3)):

- Mỗi nợ có **một số**, **một nghĩa**; **không tái dùng số**, **không đánh lại số** (số đã dùng giữ nguyên nghĩa cũ).
- Mỗi dòng đủ **mô tả + nguồn + trạng thái** để đọc mà không cần mở báo cáo cũ.
- Bốn trạng thái: `mở` · `đóng ở <task>` · `chuyển VS3` · `không làm có lý do`.
- Nợ **VS3** gom ở §5, nợ **không làm** gom ở §6, **số trùng** xử lý ở §7.

## 1. Nguồn đã quét

Máy trích: `JOURNAL/st145-trich-no.log` (lệnh + sha256 từng báo cáo + thời điểm).

| Nguồn | Dải số | Ghi chú |
|---|---|---|
| `R11-viscript.md` §5 | 1..37 | danh sách gộp đầy đủ đầu tiên (R10 §5 + ST-63..ST-75) |
| `R12-viscript.md` §5 | 1..44 | gộp đủ R11 §5 (37 mục) + ST-77..ST-81 |
| `R13-viscript.md` §5 | 45..58 | nợ mới ST-83..ST-87b |
| `R14-viscript.md` §5 | 59..67 | nợ mới ST-89..ST-91 |
| `R15-viscript.md` §5 | 68..74 | nợ mới ST-93..ST-94 |
| `R16-viscript.md` §5 | 75..77 | nợ mới ST-96..ST-98 |
| `R17-viscript.md` §5 | 78..82 | nợ mới ST-100..ST-102 |
| `R18-viscript.md` §5 | 83..91 | nợ mới ST-104..ST-107 |
| `R19-viscript.md` §5 | 92..96 | nợ mới ST-108..ST-111 |
| `R21-viscript.md` §5 | 97..100 | nợ mới vòng 21 (ST-114a..ST-115) |
| `R22-viscript.md` §5 | 101..104 | nợ mới vòng 22 (ST-116..ST-118) |
| `R23-viscript.md` §6 | 105..131 | dãy **chuẩn hoá** sau khi ba agent đợt 23 đánh số trùng |
| `R24-viscript.md` §5 | 132..141 | nợ mới đợt 24 (ST-132b/134/135/137/R24) |
| `JOURNAL/st122..st132` §nợ | 105..116 (số cũ) | nguồn gốc từng nợ + mức VS3; xem §7 |
| `QUEUE.md` | — | dòng `nợ #…` + hàng `CHỜ DUYỆT` để chốt trạng thái |
| `DECISIONS.md` | — | VD-109/122/123/124/125/126/133/135/141/144/145/146 để chốt trạng thái; **VD-149** cấp số **142/143** cho `#113b`/`#114b` (ST-148); **VD-153** cấp **#144**; **VD-154** chuyển `#143` → VS3 + cấp **#145** (ST-154) |
| `HOST-BAO-DAM.md` | — | dòng giới hạn **đường ghi** (nợ #109b/#140) |

Báo cáo **không** có dòng nợ đánh số: `R1` (chưa có sổ nợ đánh số) và `R20` (ghi “giữ nguyên R19 §5”).
`R2..R10` có danh sách nợ **riêng của từng vòng** (số 1..N, khác dãy R11 trở đi); dãy chuẩn của sổ bắt đầu ở **R11 §5**.

## 2. Quy ước số

1. Dãy số của sổ là dãy **liên tục 1..145**: 1..141 theo thứ tự R11 → R24 (R23 đã chuẩn hoá dãy 105..131),
   tiếp **142/143** cấp ngày 2026-11-17 theo **VD-149** từ hai nhãn tạm `#113b`/`#114b` (§7.3), tiếp **144** cấp theo
   **VD-153** (vế còn lại của `#133`: `read_dir` trên thư mục **có quyền ghi**) và **145** cấp theo **VD-154(c)**
   (chú thích `rt.rs` nói sai đường chặn `chan(0)`).
2. **Không tái dùng số** đã có, **không đánh lại** số cũ; nợ mới nhận số **147** trở đi.
3. Số trùng nghĩa cũ được **tách bằng hậu tố** `a/b`, **không** đổi số gốc: `#109a`/`#109b` (xem §7.1).
4. Hậu tố `a/b` **không** tạo dãy mới: `#109a` và `#109b` là hai nợ khác nghĩa của cùng số cũ 109.
5. Nguồn của mỗi mục ghi dạng `R## §5`/`R## §6`, kèm `ST-##`, `JOURNAL/<tệp>` hoặc `VD-###` khi cần chốt trạng thái.
6. Số **142/143** cấp theo **VD-149** (không tái dùng số cũ); báo cáo trích theo số mới, **kèm số cũ trong ngoặc**:
   `#142` (`#113b`), `#143` (`#114b`). Ánh xạ ghi ở §7.3.

## 3. Cách cập nhật (luật cho R25 trở đi)

- **Chỉ thêm** dòng mới ở cuối bảng (số kế tiếp); **không xoá** dòng, **không sửa số**, **không đổi nghĩa**.
- Cấp số chỉ khi có văn bản quyết định (như **VD-149**): số mới **không** tái dùng số cũ, nghĩa của số cũ **không** đổi;
  nhãn tạm cũ (§7.3) giữ làm ánh xạ, không thay số cũ.
- Đóng nợ: ghi `đóng ở <task>` **kèm nguồn** (báo cáo/hàng QUEUE/VD) — không đóng bằng suy đoán.
- Chuyển VS3: ghi `chuyển VS3` + lý do 1 dòng ở §5.
- Không làm: ghi `không làm có lý do` + văn bản quyết định (VD) ở §6.
- Báo cáo R## **không** sửa nội dung (VD-142); sai sót đính chính bằng dòng trong QUEUE + mục “Đính chính R##” ở báo cáo kế tiếp.
- Nợ phát sinh trong task: thêm số mới **ngay trong task đó**, không chờ báo cáo.

## 4. Bảng nợ chính (147 dòng: 1..146, #109 tách a/b)

| Số | Mô tả | Nguồn | Trạng thái |
|---|---|---|---|
| 1 | Shell-let chưa hạ: `let out = ls -l` qua `check` nhưng `run` báo `out` chưa bind. | R11 §5 + R12 §5 | mở |
| 2 | `match` không khớp ở ngữ cảnh giá trị vẫn đẩy `false` thay vì `VS0501` — nghi lệch interp ↔ VM, chưa dựng được ca tối thiểu. | R11 §5 + R12 §5 | mở |
| 3 | Gán lồng sâu hơn một tầng trường (`x.f[i][j]`) là `VS0412` có chủ đích. | R11 §5 + R12 §5 | mở |
| 4 | Thân `impl` chưa tyck đầy đủ — mới kiểm đích gán. | R11 §5 + R12 §5 | mở |
| 5 | Siêu trait bắc cầu mới kiểm một tầng. | R11 §5 + R12 §5 | mở |
| 6 | `impl_pairs` per-tệp: `impl` sinh từ `derives` ở tệp khác không thấy; dispatch xuyên tệp per-tệp. | R11 §5 + R12 §5 | mở |
| 7 | `co_float_sau` không soi bản ghi lồng (`Eq` derives). | R11 §5 + R12 §5 | mở |
| 8 | `derives Json` chưa sinh mã (chỉ kiểm tĩnh). | R11 §5 + R12 §5 | mở |
| 9 | `Ty::Fn` thật cho `sort_by` (đang `Ty::FnKhoa`), map khoá không-`str`, thứ tự khoá map. | R11 §5 + R12 §5 | mở |
| 10 | Nhóm nhiều pipeline + kiểu tĩnh lấy stage đầu; lambda `where` không chú thích ⇒ `{error}`. | R11 §5 + R12 §5 | mở |
| 11 | `lower/cmd.rs` 1.491 dòng — sát ngưỡng R1, cần tách trước khi thêm tính năng. (ST-106: `cmd.rs` 1491→1060) | R11 §5 + R12 §5 · R18 §6 (ST-106 tách `cmd.rs`) | đóng ở ST-106 |
| 12 | Async chưa song song thật: scheduler run-to-completion, không nhường, không GC (chu trình `Rc`); `await` lồng sát ngưỡng khung 256 | R11 §5 + R12 §5 · R23 §4 hàng 1 · VD-123 | chuyển VS3 |
| 13 | `async` trên method/`cmd` ⇒ `VS0201`; `async fn` không nhận `--tên`. | R11 §5 + R12 §5 | mở |
| 14 | `p.x += 1` là lỗi cú pháp `VS0201` (front end chặn trước tyck). | R11 §5 + R12 §5 | mở |
| 15 | Drain chỉ task `ready` (`spawn`): task `Moi` do gọi `async fn` chưa chạy ⇒ `recv` có thể `VS0505`. | R11 §5 + R12 §5 | mở |
| 16 | `close` idempotent nhưng không đánh thức ai (không wait-set). | R11 §5 + R12 §5 · VD-123 | chuyển VS3 |
| 17 | Không `select`/timeout trên kênh. | R11 §5 + R12 §5 · R23 §4 hàng 2 · VD-123 | chuyển VS3 |
| 18 | `Chan` không truyền qua biên host. | R11 §5 + R12 §5 | mở |
| 19 | `cap = 0` thực tế không dùng được (hai phía chặn đều drain rồi `VS0505`). | R11 §5 + R12 §5 · VD-123 | chuyển VS3 |
| 20 | Dung lượng âm là lỗi lúc chạy `VS0501`; `Chan` in `<kênh>` | R11 §5 + R12 §5 | mở |
| 21 | Net chỉ loopback, host khác ⇒ Err; không TLS/keep-alive (redirect + chunked đã có ở ST-104/ST-105) — phần còn lại là VS3 | R11 §5 + R12 §5 · R23 §4 hàng 3 · VD-122 | chuyển VS3 |
| 22 | Một luồng: server phục vụ tuần tự một client, không `select`/epoll; handle per-process. | R11 §5 + R12 §5 | mở |
| 23 | `net.read` 3 đối số; `std.net.connect` dạng đầy đủ chưa hạ; bỏ `is_open`; `read` trả `str` lossy; connect mặc định 1 s | R11 §5 + R12 §5 | mở |
| 24 | `vis check` chẩn đoán = exit 1; thiếu `use std.http` ⇒ `VS0501` lúc chạy (không `VS0301`); `vis check` permissive với tên cấp tệp | R11 §5 + R12 §5 | mở |
| 25 | Cap body 8 MiB, header 64 KiB; HTTP chỉ `http://`; ca net/http nằm ở `net*.rs`/`http.rs`, không trong sem | R11 §5 + R12 §5 | mở |
| 26 | LSP module-aware chỉ ở chẩn đoán (ST-80); completion/hover vẫn `phan_tich` một tệp; chẩn đoán tệp phụ không publish | R11 §5 + R12 §5 | mở |
| 27 | Chỉ full sync (`didChange` toàn văn), một luồng; EOF ⇒ exit 0 (khác spec LSP — cần cho test/CI). | R11 §5 + R12 §5 | mở |
| 28 | Completion chưa theo ngữ cảnh sau `.`/kiểu; hover chưa field; biến gộp mọi phạm vi. | R11 §5 + R12 §5 | mở |
| 29 | Lower/tyck vẫn chạy khi lex/parse lỗi ⇒ có thể thêm nhiễu chẩn đoán | R11 §5 + R12 §5 | mở |
| 30 | Chưa resolver dải: phụ thuộc ghim phiên bản chính xác (đã có ở ST-128); phần còn lại — ký số, registry HTTP — chuyển #118 | R11 §5 + R12 §5 · VD-125 | đóng ở ST-128 |
| 31 | Lock chưa lưu `entry` (dùng quy ước); lock chỉ tìm ở `main_dir`; `.vis/store` không GC. | R11 §5 + R12 §5 | mở |
| 32 | Chưa `pkg remove`/`update` (đã có ở ST-90/ST-102); `use pkg.*` chỉ nạp tệp có trong lock; chưa kiểm toàn vẹn cây ngoài `file_sha` | R11 §5 + R12 §5 | mở |
| 33 | Siết mặc định vẫn là nợ: `--strict-caps` opt-in; muốn siết mặc định phải chuyển toàn bộ fixture/host suite sang khai `cap` | R11 §5 + R12 §5 · R23 §4 hàng 4 · VD-124 | chuyển VS3 |
| 34 | Landlock chỉ probe ở mọi nền; Linux chưa chạy thật; `vis-cli` chưa dùng crate `vis-host-linux` (đã đóng khi Landlock thật chạy 13/13 ca) | R11 §5 + R12 §5 · R23 §4 hàng 4 · R24 §4 hàng 4 | đóng ở ST-132/132b |
| 35 | Ca sem `cap/` mới kiểm tyck; giá trị runtime chỉ đến từ tham số `main` | R11 §5 + R12 §5 | mở |
| 36 | Chưa có runner Windows/wasm/Linux ⇒ mọi ô chỉ là check biên dịch chéo; `vis-cli` trên wasm không chạy; chưa đo build/link Windows (ST-131/ST-136: Linux chạy thật, Windows đã đo; wasm chỉ check theo VD-121) | R11 §5 + R12 §5 · R23 §4 hàng 7 · VD-121/VD-136 | đóng ở ST-131/ST-136 |
| 37 | Net không nằm trong suite host (24 ca riêng); suite phụ thuộc lệnh macOS (`/bin/echo`, `/bin/cat`, `/usr/bin/grep`…) | R11 §5 + R12 §5 | mở |
| 38 | `.vish` không chuẩn hoá thụt lề (ST-77): `vis fmt` giữ thụt lề gốc của khối `cmd`, không tự chuẩn hoá độ sâu; muốn chuẩn hoá phải sửa 68 fixture `.vish` | R12 §5 | mở |
| 39 | `usage_explain` chỉ tiếng Việt (ST-78): trợ giúp in “7 khối, 32 mã” đếm động nhưng không theo `--lang`; `--list` vẫn in kèm cột severity | R12 §5 | mở |
| 40 | `assert` chỉ dạng gọi hàm (ST-79): `assert(đk)`; chưa hỗ trợ `assert đk` như tài liệu; mã lỗi dùng `VS0501` “assert thất bại” (không thêm mã mới). (ST-83: đã thêm `assert đk`) | R12 §5 · R13 §5 #45 | đóng ở ST-83 |
| 41 | Runner `test` biến đổi văn bản (ST-79): mỗi `test fn` chạy bằng bản sao tạm đổi thành `fn main`; câu lệnh cấp tệp chạy lại cho mỗi test; `async fn` trong thân `test` không hỗ trợ | R12 §5 | mở |
| 42 | LSP completion/hover chưa module-aware (ST-80): chỉ chẩn đoán đi đường module; gợi ý/hover vẫn một tệp; xem thêm nợ 26. | R12 §5 | mở |
| 43 | Siết mặc định chưa làm: `--strict-caps` opt-in; `--sandbox-check`/`--dry` không áp cờ | R12 §5 · VD-124 (ST-130 siết mặc định khi dùng gói) | chuyển VS3 |
| 44 | `fs.exists` ngoài quyền trả `false` im lặng thay vì trap `VS0604` như read/write/list (ST-83: nay trap `VS0604`) | R12 §5 · R13 §5 (đầu mục) · VD-109 | đóng ở ST-83 |
| 45 | `assert` câu lệnh chỉ một biểu thức, không message (ST-83): `assert đk`; chưa `assert đk, "msg"`, chưa đếm/nhãn; mã lỗi dùng `VS0501` (không mã mới). | R13 §5 | mở |
| 46 | Biến tên `assert`: `assert` là từ khoá câu lệnh; dùng `assert` làm định danh (biến/hàm) bị nhập nhằng — chưa có luật đặt tên/chẩn đoán riêng. | R13 §5 | mở |
| 47 | Trợ giúp `explain` dịch tay 3 ngôn ngữ (ST-83): câu vi/en/ja viết trong `main.rs`; thêm mã mới phải sửa 3 chỗ; chưa có nguồn dữ liệu chung với `vis explain` | R13 §5 | mở |
| 48 | `is_pub` mới giữ cho `fn` (ST-84): HIR `HRec/HEnum/HAlias/HConst/HTrait` không giữ cờ `pub` ⇒ LSP gợi ý cả item không `pub` của module; cần thêm cờ vào HIR | R13 §5 | mở |
| 49 | Completion chưa theo ngữ cảnh sau `.`: vẫn lọc tiền tố trên toàn bộ item; sau `x.` chưa gợi ý field/method theo kiểu — phần còn lại chuyển #65 | R13 §5 · R14 §5 (đầu mục) | đóng ở ST-91 |
| 50 | Hover chưa field (ST-84): hover biến/hàm có; field bản ghi/chấm chưa. | R13 §5 | mở |
| 51 | `--indent-report` fallback ước lượng (ST-85): khi thân `cmd` chỉ là lệnh, đếm theo dòng (`cmd`/`impl`/`type` mở khối) chứ không theo CST đầy đủ | R13 §5 | mở |
| 52 | `dong_thut_sai` heuristic văn bản (ST-85b): dấu `:` cột 1 trong chuỗi/comment có thể bị coi mở khối; khối một dòng `cmd f(): x` không mở khối; nối dòng `\` ngoài ngoặc chưa hỗ trợ | R13 §5 | mở |
| 53 | `khai::tach` chưa dọn gốc (ST-85b): vẫn coi dòng thụt là tiền tố khai báo; nay có `VS0201` chặn nhưng logic tách chưa sửa tận gốc. | R13 §5 | mở |
| 54 | `topmix2` đổi hành vi (ST-85b): dòng cấp tệp thứ hai thụt lề trước đây chạy như lệnh ngoài, nay báo `VS0201` (đúng §9.5 nhưng đổi hành vi với tệp sai thiết kế) | R13 §5 | mở |
| 55 | Bộ sinh fuzz mẫu cố định (ST-86): 8 nhóm là mẫu cố định, chưa ghép tổ hợp sâu; kênh chỉ cặp cân bằng; chưa sinh `use pkg.*` thành công; `@`/`(write)` mới ở mức cú pháp | R13 §5 | mở |
| 56 | 7 ca `.vish` khác hai đường có chủ đích: lệnh ngoài chạy ở interp nhưng VM báo `VS0604` (VM chưa có host `Ngoai`) (ST-89: VM có host `Ngoai` ⇒ 0 SKIP) | R13 §5 · R14 §5 (đầu mục) | đóng ở ST-89 |
| 57 | Frontend chạy mỗi lần (ST-87b): mọi lần `--vm` vẫn parse/lower/check (~1 ms tệp nhỏ) rồi mới tra cache codegen; không còn đường bỏ frontend | R13 §5 | mở |
| 58 | Ca chống nhánh chết đọc mã nguồn (ST-87b): `khong_con_fast_path_khoa_v1` grep `main.rs`/`cache.rs`; brittle nếu tách hàm/đổi tên. | R13 §5 | mở |
| 59 | `--vm` vẫn một luồng (ST-89): host `Ngoai` không đổi scheduler; lệnh ngoài chạy tuần tự, không có song song/async cho process. | R14 §5 | mở |
| 60 | Lệnh ngoài chưa qua `cap` (ST-89/ST-90): `exec.*`/`Ngoai` chạy không cần khai quyền; chưa có `cap[Exec]` — phần còn lại chuyển #68 | R14 §5 · R15 §5 (đầu mục) | đóng ở ST-93 |
| 61 | `vis test` dùng `--no-cache` (ST-89): mọi ca test chạy `--vm --no-cache` để tránh cache ảnh hưởng; chưa có chế độ test cache ấm. | R14 §5 | mở |
| 62 | Lock cũ không tự nâng cấp (ST-90): lock thiếu `entry` chỉ fallback; chỉ khi `add`/`remove` ghi lại mới có `entry`; không có `pkg migrate`/chuẩn hoá lock (ST-129: lock định dạng 2 + migration theo VD-125) | R14 §5 · VD-125 · R24 §4 hàng 6 | đóng ở ST-129 |
| 63 | `entry` chưa kiểm đuôi (ST-90): chỉ kiểm đường dẫn tương đối + có trong `files`; chưa bắt buộc `.vis`/tồn tại đúng kiểu tệp. | R14 §5 | mở |
| 64 | `remove` không dọn dependency mồ côi (ST-90): xoá `http` để lại `util` trong lock/store (đúng “chỉ xoá tên được gọi”); chưa có `pkg remove --prune`/GC tham chiếu | R14 §5 | mở |
| 65 | LSP chỉ receiver đơn giản: `a.b.`, `(x).`, `f().`, generic/`list[P]`/`Opt[P]`, `impl` cho alias/enum chưa mở thành viên; chưa có bảng phương thức `std.str` | R14 §5 | mở |
| 66 | `entry`/thành viên chưa cache (ST-91): mỗi completion sau `.` parse/lower lại với sentinel `__vis` (~1 lần phân tích); chưa cache theo phiên/tệp. | R14 §5 | mở |
| 67 | `self` in gọn không kiểu (ST-91): chữ ký method hiển thị `fn show(self) -> str` (bỏ `: {error}`); chưa suy kiểu `self` thật cho hover/detail. | R14 §5 | mở |
| 68 | `cap[Proc]` không allowlist: quyền chạy mọi tiến trình, không giới hạn danh sách/đường dẫn; chưa áp cho tiến trình con (ST-97: `--grant p=<lệnh>` = allowlist) | R15 §5 · QUEUE ST-97 | đóng ở ST-97 |
| 69 | `.vish` có cả pipeline cấp tệp và `fn main` ⇒ HIR bỏ `main` để pipeline chạy; `main` chỉ khai `cap` (đổi ngữ nghĩa) | R15 §5 | mở |
| 70 | `pass` nhận diện theo tên (ST-93b): thân `main` chỉ có `HStmt::Expr(Var("pass"))` mới được coi là rỗng; chưa có kiểm “khai báo kiểu trong thân” | R15 §5 | mở |
| 71 | Không rendezvous: `chan(0)` bị từ chối runtime `VS0505` nhưng exit 1, không phải 2; chưa có wait-set | R15 §5 · R23 §4 hàng 2 · VD-123 | chuyển VS3 |
| 72 | Drain không wait-set: join cuối chạy task `Moi` theo hàng đợi, không có cơ chế chờ `recv`/`send` đúng nghĩa | R15 §5 · VD-123 | chuyển VS3 |
| 73 | `--grant` không áp cho `Proc` (ST-93): `--grant tên=path` chỉ có nghĩa Dir/File/Net; `cap[Proc]` không có tham số đường dẫn nên không nhận grant. (ST-97: `cap[Proc]` đã nhận `--grant p=`) | R15 §5 · R16 §6 (ST-97) | đóng ở ST-97 |
| 74 | Tiến trình con không sandbox (ST-93): `cap[Proc]` cho phép spawn, nhưng tiến trình con không bị Landlock/seccomp/namespace; mới chỉ kiểm ở biên host | R15 §5 | mở |
| 75 | Quét sổ mã chỉ `crates/*/src/**`: không quét test/script/`build.rs`; `DU_PHONG` rỗng; chưa đối chiếu mã trong tài liệu/spec | R16 §5 | mở |
| 76 | Allowlist `cap[Proc]` còn rộng: khớp basename (ST-123 đã đổi sang canonical), không giới hạn đối số, không hỗ trợ dạng `:` | R16 §5 | mở |
| 77 | Soak chưa đủ 30 phút + phụ thuộc PATH: 8 vòng 818 s; script giả định `cargo` ở `~/.cargo/bin`; chưa soak Linux/Windows | R16 §5 | mở |
| 78 | JSON thiếu `severity` (ST-101): `check --json` chỉ `code/line/col/message`; chưa phân biệt warning/error; chưa có `source`/`range` UTF-16 như LSP. (ST-110: đã có `severity`) | R17 §5 · R19 §4 (ST-110) | đóng ở ST-110 |
| 79 | Chẩn đoán tệp phụ dưới `file` chính (ST-101): `files[].diagnostics` gộp theo tệp đối số, không có trường `phu`/`file` gốc của chẩn đoán (ST-110: `files` theo tệp nạp) | R17 §5 · R19 §4 (ST-110) | đóng ở ST-110 |
| 80 | Cắt message 200 ký tự (ST-101): `test --json` cắt theo ký tự (không byte), thêm “…” | R17 §5 | mở |
| 81 | Cache LSP global (`thread_local`) + tệp dùng `pkg` luôn nạp lại; `didClose` xoá toàn bộ cache; hover chỉ khi tên duy nhất | R17 §5 | mở |
| 82 | `pkg update` chưa đủ resolver/blob: chỉ ghi gói đích, không cập nhật phụ thuộc; blob cũ không GC; registry cục bộ | R17 §5 | mở |
| 83 | Chưa TLS/HTTP2/keep-alive: `http.get` chỉ `http://`, một request/kết nối, không nén. | R18 §5 · R23 §4 hàng 3 · VD-122 | chuyển VS3 |
| 84 | Redirect chỉ GET/không cookie-auth: 303 không đổi POST, không giữ `Cookie`/`Authorization`; không giới hạn host đích ngoài loopback (vẫn `tach_url` chặn) | R18 §5 | mở |
| 85 | Chunk trailer bị bỏ qua: dừng ở chunk `0`, không đọc/không kiểm trailer. | R18 §5 | mở |
| 86 | `Location` tương đối chưa resolve `..`/query-only: chỉ ghép thư mục hiện tại; `../x`/`?q` chưa chuẩn. (ST-109: `Location` đủ `..`/query-only/fragment) | R18 §5 · R19 §6 (ST-109) | đóng ở ST-109 |
| 87 | `--timeout` chỉ `test fn`: fixture `.out/.err` không có timeout; không timeout theo ca/bộ | R18 §5 | mở |
| 88 | `--timeout` không kill cây cháu: chỉ kill `vis run`; cháu (nếu có) có thể sống. | R18 §5 | mở |
| 89 | Pipe stdout/stderr có thể nghẽn: `wait_with_output` sau khi con kết thúc; ca in quá buffer có thể treo. (ST-139: 3/3 unit + 7/7 tích hợp xanh) | R18 §5 · QUEUE ST-139 · PR #24 · `ef6489d1` · JOURNAL/st139-do-truoc.log | đóng ở ST-139 |
| 90 | `lower.rs` 1482 vẫn max R1: ST-106 chỉ tách `cmd.rs`; muốn max ≤1200 phải tách `lower.rs`. (ST-108: `lower.rs` 1482→993) | R18 §5 · R19 §6 (ST-108) | đóng ở ST-108 |
| 91 | `ha_stage` 782 vẫn lớn: chưa tách tiếp; `cmd.rs` còn 1060. | R18 §5 | mở |
| 92 | Span HIR chưa có file-id (correctness): mỗi tệp hạ với span bắt đầu 0 ⇒ câu lệnh cấp tệp chính có thể bị quy sang module (ST-114a/b: `Span.file`) | R19 §5 · R21 §6 · R22 §1 | đóng ở ST-114a/b |
| 93 | `HEAD` không theo redirect: trả 302 + `Location`, không đọc thân (đã chọn, ghi tài liệu); chưa `HEAD` keep-alive/HTTP2. | R19 §5 | mở |
| 94 | `FUNCS` 52 cần review: thêm `http.head`/`head_timeout` và `Ty::ResultHead` (`map[str, {error}]`); test đếm 50→52 đã sửa kèm lý do | R19 §5 | mở |
| 95 | `cmd.rs` 1060 vẫn max R1: dưới 1.200 nhưng `ha_stage` 782 dòng chưa tách; tách tiếp nếu muốn <900. | R19 §5 | mở |
| 96 | `check --json` nhiều đối số có thể lặp module; `files` chưa có metadata `main`/`phu`; TAP vẫn báo chẩn đoán đầu | R19 §5 | mở |
| 97 | Span tổng hợp `file=0`: vài node do lower sinh dùng `Span::new` mặc định ⇒ chẩn đoán có thể về tệp chính; chưa audit | R21 §5 | mở |
| 98 | LSP chưa publish chẩn đoán tệp phụ theo file-id — hành vi LSP không đổi; mới có module-aware. | R21 §5 | mở |
| 99 | Bytecode `Span` không mang file — `VBC1` giữ 53; nếu cần debug đa tệp trong VM phải bump format. | R21 §5 | mở |
| 100 | `parse/mod.rs` 1452 dòng gần trần R1 1500; thêm chức năng parse đa tệp dễ vượt — cần tách trước khi mở rộng | R21 §5 | mở |
| 101 | `lower/cmd.rs` 1060 — dưới luật R1 1500 nhưng trên mục tiêu nội bộ ≤1000; `parse/cmd.rs` 1007 cũng vậy; cần task tách riêng nếu muốn ≤1000 toàn cục. | R22 §5 | mở |
| 102 | `main.rs` 591 dòng nhưng `run_run` vẫn 352 dòng trong main (giữ `cache::khoa_multi` cho ca `cache_fastpath`) | R22 §5 | mở |
| 103 | Span tổng hợp `file=0` — node do lower sinh vẫn mặc định tệp chính; chưa audit hết. | R22 §5 | mở |
| 104 | LSP chưa publish chẩn đoán tệp phụ cho tài liệu chính; mới bảo đảm tài liệu đang mở đúng tệp. | R22 §5 · VD-126 | không làm có lý do |
| 105 | `HSym` của builtin vẫn `Span::new(0,0)` — bảng ký hiệu dùng chung, builtin không thuộc tệp nào; cần chốt `Option<Span>`/`u16::MAX` | R23 §6 · ST-122 | mở |
| 106 | `RecDef`/`FieldDef` của prelude + `cmdsig` vẫn `file=0`; audit chưa phủ `TypeTable` | R23 §6 · ST-122 | mở |
| 107 | `test/sem` không chứa được ca đa tệp (5 harness một tầng + `vistest::gom` đệ quy) | R23 §6 · ST-122 | mở |
| 108 | `Command::new(tên)` để OS phân giải `PATH` lần nữa sau khi kiểm | R23 §6 · ST-123 | mở |
| 109a | `next_block()` trả `801` trong khi khối `07xx` vẫn còn chỗ (`704` đã cấp): hàm là “khối mới”, không phải “số kế tiếp” | JOURNAL/st128-resolver.md §8 · ST-128 · QUEUE ST-128 · R23 §6 #116 | chuyển VS3 |
| 109b | Đường GHI vẫn kiểm-rồi-mở (`FsGuard::write`): đổi symlink giữa `kiem_fs` và `std::fs::write` có thể truncate/ghi ngoài root; sửa cần `O_NOFOLLOW`/`openat` = VS3; **dòng giới hạn đã có trong `HOST-BAO-DAM.md`**; sửa kỹ thuật thuộc VS3 | R23 §6 · ST-124 · R24 §5 #140 · JOURNAL/st124-review.md §7 · HOST-BAO-DAM.md §4 | mở |
| 110 | VM không đưa `argv` vào tham số không-`cap` của `main` (interp `list[str]`, VM `()`) — **ST-157**: VM chạy thẳng `main` (bỏ qua "vỏ" `Call(main, 0); Halt` của codegen) và đặt `argv` vào mọi tham số **không-`cap`**, `Value::Cap` đúng chỉ số (ST-72b); ca mới `vm_main_argv.rs` 4 ca **so byte** hai đường | R23 §6 · ST-124 · ST-157 · `JOURNAL/st157-do-truoc.log` · QUEUE ST-157 | đóng ở ST-157 |
| 111 | tyck chấp nhận `let e: cap[Dir] = json.decode(...)` | R23 §6 · ST-124 | mở |
| 112 | Thực tế có 2 bộ escape JSON (`json_esc` + `json_tai`); VD-130e “nguồn duy nhất” chưa đúng | R23 §6 · ST-124 | mở |
| 113 | `TEN_THE` không ràng buộc lúc biên dịch | R23 §6 · ST-126 · JOURNAL/st126-dong-bang-api.md §8 | chuyển VS3 |
| 114 | hash nguồn VBC1 ≠ khoá cache (hai giá trị khác nhau, không đối chiếu ở đâu) | R23 §6 · ST-126 · JOURNAL/st126-dong-bang-api.md §8 | chuyển VS3 |
| 115 | quy ước "điền 0" khi so sánh thiếu thành phần phiên bản | R23 §6 · ST-128 · JOURNAL/st127-semver.md §8 | chuyển VS3 |
| 116 | `next_block()` trả 801 trong khi khối 07xx còn chỗ ⇒ cấp số mã mới phải chọn tay | R23 §6 · ST-128 · JOURNAL/st128-resolver.md §8 | chuyển VS3 |
| 117 | `@1.2.3` = ghim, khác "không toán tử ⇒ `^`" | R23 §6 · ST-128 | mở |
| 118 | yanked, registry mạng, chữ ký (VS3) | R23 §6 · ST-128/129 · JOURNAL/st128-resolver.md §8 | chuyển VS3 |
| 119 | lock không ghi quan hệ phụ thuộc giữa gói | R23 §6 · ST-129 · JOURNAL/st129-lock.md §9 | chuyển VS3 |
| 120 | `vis test`/host suite chưa áp strict-caps | R23 §6 · ST-130 · VD-133 | chuyển VS3 |
| 121 | `pipeline.rs` bám tiền tố thông điệp grep (lệch 1 §4.4) (ST-134: `pipeline` 16/16 hai nền) | R23 §6 · ST-131 · R23 §4.4 · QUEUE ST-134 | đóng ở ST-134 |
| 122 | `sandbox.rs:59` chưa `cfg` theo nền (lệch 3) (ST-134: `sandbox` 6/6 hai nền) | R23 §6 · ST-131 · R23 §4.4 · QUEUE ST-134 | đóng ở ST-134 |
| 123 | runner thiếu `wasm32` target (lệch 5) (ST-134: `setup.sh` cài `wasm32`, targets 18/18) | R23 §6 · ST-131 · R23 §4.4 · QUEUE ST-134 | đóng ở ST-134 |
| 124 | `vish_cmd_body.rs:312` đường dẫn thư mục tạm (lệch 6) (ST-134: `vish_cmd_body` 16/16 hai nền) | R23 §6 · ST-131 · R23 §4.4 · QUEUE ST-134 | đóng ở ST-134 |
| 125 | `--sandbox` chưa cấp `EXECUTE` cho `/lib*` ⇒ lệnh ngoài không chạy được dưới sandbox | R23 §6 · ST-132 | mở |
| 126 | `unicode-ident` (`(MIT OR Apache-2.0) AND Unicode-3.0`, build-time) ngoài allowlist hẹp — cần Agent 0 quyết (VD-135: hợp lệ theo QD §9.3) | R23 §6 · ST-132 · VD-135 · QUEUE đính chính 2026-11-16 | đóng ở ST-132 (VD-135) |
| 127 | `ap_dung` mới dùng `ABI::V1`; ABI 8 có thêm `REFER`/`TRUNCATE`/`IOCTL_DEV` chưa xét | R23 §6 · ST-132 · JOURNAL/st132-landlock.md §7 | chuyển VS3 |
| 128 | RÒ RỈ TOCTOU trên Linux 798/50 000 — lỗi sản phẩm, ưu tiên cao nhất đợt 24 (ST-137: 0/50 000 trên `ps` và macOS) | R23 §6 · ST-133 · VD-141 · QUEUE ST-137 | đóng ở ST-137 |
| 129 | Cùng danh sách số dòng khối tài liệu bị chép ở 4 tệp test (`check.rs`, `fmt.rs`, `doc_fixtures.rs`, `hir.rs`) — **ST-158**: gom về **một nguồn** `viscript/test/doc_excluded.rs` (`const DOC_EXCLUDED: [usize; 7]`, không đổi số); 4 tệp test dùng `include!("../../../test/doc_excluded.rs")`; vis-syntax dùng bản **6** khối dẫn xuất `DOC_EXCLUDED_SYNTAX` (khối 666 parse sạch từ ST-27); 4 target trước/sau giống: 13/13/3/10 | R23 §6 · ST-133 · ST-158 · `JOURNAL/st158-truoc.log` · `JOURNAL/st158-sau.log` · `JOURNAL/st158-gate-macos.log` · QUEUE ST-158 | đóng ở ST-158 |
| 130 | Linux chưa chạy `vis test`/differential/cache (mới có `cargo test` + host suite) (ST-142: 447/447 hai đường, `lech=0`) | R23 §6 · ST-133 · QUEUE ST-142 · JOURNAL/st142-ps-vis-test.log · JOURNAL/st142-ps-diff-100k.log · JOURNAL/st142-ps-cache.log | đóng ở ST-142 |
| 131 | hàng `ST-132b` do Agent 2 tự thêm vào QUEUE (ngoài ủy quyền) — cần Agent 0 nhận hoặc xoá (đính chính: hàng do Agent 0 mở, không phải Agent 2 tự thêm) | R23 §6 · ST-133 · R24 §1 hàng 5 | đóng ở ST-00 (đính chính R23/R24 §1) |
| 132 | 3 ca âm dựng helper bằng `rustc`; thiếu `crt-static` ⇒ đỏ vì môi trường — **đóng ở ST-166**: đo được **6 ca** dùng chung `dung_helper` đỏ khi PATH thiếu `rustc`/`cc` (`linker 'cc' not found`); sửa **chỉ phần dựng helper** (`duong_rustc()`, `path_dung_helper()`, `+crt-static` **chỉ Linux**) ⇒ trên `ps` **15/15** ở cả 3 môi trường (chuẩn, đã-từng-đỏ, PATH rỗng); mac: phần dựng helper xanh (`rustc -O` exit 0), **suite landlock vẫn không biên dịch trên mac** vì `landlock 0.4.7` Linux-only ⇒ **câu hỏi riêng cho A0** (có `cfg`-hoá dependency trong `Cargo.toml` không); `vis-targets` 18/18 — **A0 XÁC NHẬN đóng (VD-175)** | R24 §5 · ST-132b · ST-166 · `JOURNAL/st166-*.log` | **đóng ở ST-166** |
| 133 | Ca `read_dir` mới kiểm luật chỉ-đọc, chưa kiểm luật ghi — **ST-150 đã bù**: ca `con_ghi_vao_thu_muc_chi_doc_bi_kernel_chan` (ca 14) cho tiến trình con **ghi** vào thư mục chỉ-đọc ⇒ `EACCES`, kèm đỏ-trước + đối chứng dương cùng ca + hai luật trong **một** ruleset. Biến thể `read_dir` trên thư mục **có quyền ghi** (ST-132b §7) **không** thuộc phạm vi hàng ST-150 — đề nghị cấp số ở đợt sau, chưa đưa vào bảng này | R24 §5 · ST-132b · QUEUE ST-150 · `JOURNAL/st150-landlock-ps.log` · `JOURNAL/st150-gate-macos.log` | đóng ở ST-150 |
| 134 | `setup.sh` cài thêm `x86_64-pc-windows-msvc` (ngoài `wasm32`) để đạt 18/18 — cần Agent 0 chuẩn thuận (VD-146(1) + ST-144: 2/2 target đã cài) | R24 §5 · ST-134 · VD-146(1) · QUEUE ST-144 | đóng ở ST-144 |
| 135 | `vis-linux` phải sync cả `vis-host-wasm/`; chưa có kiểm tự động danh sách sync — **ST-151 đã kiểm**: danh sách là **một nguồn** (`SYNC_DIRS`/`SYNC_FILES` ở đầu tệp, `sync_tree()` chỉ đọc hai biến) + **`--check-sync`** (8/8 mục, exit **0** đủ · **2** thiếu, không ssh/rsync) + **9 ca** `crates/vis-cli/tests/sync_list.rs` (bỏ một mục ⇒ đỏ) | R24 §5 · ST-134 · QUEUE ST-151 · `JOURNAL/st151-do-truoc.log` · `JOURNAL/st151-gate-macos-lan2.log` | đóng ở ST-151 |
| 136 | Phân loại thân `cmd` theo tên lệnh: biến trùng tên lệnh muốn làm biểu thức bị hiểu là lệnh (đường thoát `( … )`), chưa có ca khoá — **ST-149 đã sửa**: biến bind từ ngoài (`let`/`var`/`const` cấp tệp + tham số `cmd`/`fn`) **che** tên lệnh ⇒ thân là **biểu thức**; không bind ⇒ luật ST-134 nguyên vẹn | R24 §5 · ST-134 · QUEUE ST-149 · `JOURNAL/st149-do-truoc.log` · `JOURNAL/st149-tai-hien.log` | đóng ở ST-149 |
| 137 | macOS vẫn dùng bước xác nhận hai-syscall của ST-124 (không có `/proc`, std an toàn không có fd→path) — đo 0/50 000 nhưng không bảo đảm theo cơ chế | R24 §5 · ST-137 · VD-145 | chuyển VS3 |
| 138 | `/proc` phải đọc được trên Linux, nếu không ⇒ `VS0604` (hành vi đúng, nhưng người vận hành phải biết) — **đóng ở ST-165**: đo trên `ps` thấy `/proc/self/{status,cmdline}` **đọc được**, không `VS0604`; cái sai là **chẩn đoán** (không nêu đích `/dev/null`, không phân biệt `pipe:/socket:`); sửa `quyen.rs` (`kiem_canon` nêu đích phân giải + nhánh phi-đường-dẫn; `xac_nhan_handle` thêm hướng dẫn procfs) — **Agent 1 ghi nhận, CHỜ A0 XÁC NHẬN** | R24 §5 · ST-137 · ST-165 · `JOURNAL/st165-proc-sau.log`, `st165-proc-khong-doc-duoc.log` | **đóng ở ST-165** |
| 139 | Ca `symlink_canonicalize_hai_phia` đỏ trên Linux vì coreutils multicall — cần VD thứ 3 theo VD-132 hoặc cài `echo` thật trên runner (VD-144 + ST-140: ca xanh hai nền) | R24 §5 · ST-135 · VD-144 · QUEUE ST-140 · JOURNAL/st140-red-ps.log · JOURNAL/st140-cargo-test-ps.log | đóng ở ST-140 |
| 140 | Nợ cũ #109b (đường ghi vẫn kiểm-rồi-mở) vẫn mở; cần một dòng giới hạn trong `HOST-BAO-DAM.md` (Agent 0) — dòng đó đã có từ commit 69112900; **sửa kỹ thuật thuộc VS3** | R24 §5 · HOST-BAO-DAM.md §4 | mở |
| 141 | Mục điều hướng: nợ cũ #121..#124, #128, #129, #130 đã xử lý hoặc chuyển VS3 — xem R23 §6 | R24 §5 | không phải nợ (điều hướng) |
| 142 | **`pkg update <tên>` ghim mọi gói khác** (`=<version>`): gói được update cần nâng dep chung thì resolver báo `VS0704` thay vì nâng; cần quyết định chính sách (trước là nhãn tạm `#113b`) | JOURNAL/st129-lock.md §9 · QUEUE ST-129 · VD-149 | mở |
| 143 | **`NamePolicy::Strict` bật cho mọi tệp có `use`** (`modules.rs:606`), không chỉ tệp trong cây module; từng che lỗi VD-124 (trước là nhãn tạm `#114b`) | JOURNAL/st130-siet-caps.md §9 · QUEUE ST-130 · VD-133 · VD-149 · **VD-154(b)** | chuyển VS3 — VD-154(b): chuyển VS3 vì ca st130b xanh |
| 144 | **Vế 2 của `#133`**: chưa có ca **dương** `read_dir` trên thư mục **CÓ quyền ghi** ở tiến trình con (ST-132b §7) — **ST-154 đã bù**: ca 15 `con_read_dir_thu_muc_co_quyen_ghi_van_liet_ke_duoc` trong `vis-host-linux/tests/landlock.rs` (**MỘT** ruleset **hai** luật: `trong` chỉ-đọc + `trong-ghi` ghi; `co.txt` ghi **trước** khi áp ruleset; helper `ls trong-ghi` ⇒ `exit 0` + danh sách có `co.txt`; helper `ghi` trong `trong-ghi` ⇒ `exit 0` rồi `ls` thấy `co.txt` + `moi.txt`; âm cùng ca: `ls ngoai` ⇒ `exit 7` + `Permission denied`) | JOURNAL/st154-landlock-ps.log · QUEUE ST-154 · VD-153 | đóng ở ST-154 |
| 145 | **Chú thích `rt.rs` nói sai đường chặn `chan(0)`**: `rt.rs:128-129` nói literal `chan(0)` bị **tyck** chặn (exit 2); thực tế tyck **không** kiểm `cap` (`tyck/expr/kenh.rs:16-33` — `kiem_chan_new` chỉ kiểm `n` là `int`), **runtime** mới chặn (exit 1, `kenh_cap0.rs:57-68`); sửa chú thích hoặc thêm kiểm tyck | JOURNAL/st155-vs3-dieu-tra.md §2 · VD-154(c) | mở (xử khi có task VS3 kênh) |
| 147 | **mac chậm hơn ps** — đo lại có khoá (ST-176, máy rảnh): mac **1489 s** (số chuẩn, `lock=yes`, load 1,32→1,81) vs ps **261 s** (clone sạch, `lock=yes`) = **5,70×** > 3× ⇒ **chưa đóng**. Nguyên nhân: **tạo tiến trình + syscall macOS**, không phải CPU: `check` **15,5×** (190,0/12,3 s) chiếm 59 % gate mac; `report-check.py` **22,8×** (111,9/4,9 s — gọi `git` từng báo cáo); `vis-dev rules` **8,3×** (49,3/6,0 s — một `wc`/`grep` mỗi tệp); chi phí tạo tiến trình trần **9,6×** (15,4/1,6 ms); ca `#146` 50 000 vòng **20,8×** (8,51/0,41 s). 10 suite chênh lớn nhất = 77 % tổng chênh (`cache_khong_che` +23,1 s, `vis` +10,1, `diag_cmp` +10,0, `host_conformance` +7,2 …) | ST-167 · VD-177 · ST-176 | **mở** — số chuẩn mac **1489 s**; đề xuất A0: (i) gộp `report-check`/`rules` để **không tạo tiến trình mỗi tệp** (đo lại sau), (ii) ghi nhận macOS phạt syscall là **giới hạn nền**, không phải lỗi mã |
| 146 | **Ca đua `dua_doi_symlink_lap_co_gioi_han` đỏ 1/50 000** (R28 §5 #1) — **đóng ở ST-174 (VD-176, phương án A)**: TOCTOU thật chỉ macOS (`std::fs::metadata` đi theo symlink sau `canonicalize`); vá **1 dòng** `symlink_metadata` (`quyen.rs:513`) + **điểm chèn `#[cfg(all(test, unix))]`** giữa `canonicalize` và bước kiểm; **chứng minh tất định**: ca A đỏ **100/100** trên mã cũ (`ro_ri=100`), sau vá **100/100 xanh** trên **mac và ps**, ca B đối chứng `TRONG` 100/100, **ca âm** bỏ điểm chèn ⇒ “không chạy được điểm chèn”; **6 000 000 vòng** mac `ro_ri=0`; gate mac **3 lần liên tiếp PASS** L0 **1184/0**; `cfg(test)` không lọt build (`strings`/`nm` = 0); giới hạn **hardlink** ghi `HOST-BAO-DAM.md` | R28 §5 #1 · VD-158 · VD-176 · ST-163 · ST-174 · `JOURNAL/st174-*.log`, `st174-ghi-chu.md` | **đóng ở ST-174** |

## 5. Nợ chuyển VS3 (21 mục)

| Số | Lý do chuyển VS3 (1 dòng) | Nguồn |
|---|---|---|
| 12 | VD-123: async song song thật + GC là dự án VS3 có tài liệu thiết kế riêng | R11 §5 + R12 §5 · R23 §4 hàng 1 · VD-123 |
| 16 | VD-123: wait-set/đánh thức khi `close` thuộc rendezvous — VS3 | R11 §5 + R12 §5 · VD-123 |
| 17 | VD-123: `select`/timeout trên kênh là VS3 | R11 §5 + R12 §5 · R23 §4 hàng 2 · VD-123 |
| 19 | VD-123: `chan(0)`/rendezvous là VS3; VS2 từ chối bằng `VS0505` | R11 §5 + R12 §5 · VD-123 |
| 21 | VD-122: TLS/keep-alive chưa có trong VS2, xét rustls ở VS3 | R11 §5 + R12 §5 · R23 §4 hàng 3 · VD-122 |
| 33 | VD-124: siết mặc định toàn cục là nợ VS3 | R11 §5 + R12 §5 · R23 §4 hàng 4 · VD-124 |
| 43 | VD-124: như #33; ST-130 chỉ siết mặc định khi chương trình dùng gói | R12 §5 · VD-124 (ST-130 siết mặc định khi dùng gói) |
| 71 | VD-123: rendezvous + wait-set là VS3 (VS2 giữ `chan(0)` ⇒ `VS0505` exit 1) | R15 §5 · R23 §4 hàng 2 · VD-123 |
| 72 | VD-123: wait-set cho drain là VS3 | R15 §5 · VD-123 |
| 83 | VD-122: TLS/HTTP2/keep-alive là VS3 | R18 §5 · R23 §4 hàng 3 · VD-122 |
| 109a | `JOURNAL/st128` §8 ghi mức VS3; trùng nội dung #116 | JOURNAL/st128-resolver.md §8 · ST-128 · QUEUE ST-128 · R23 §6 #116 |
| 113 | `JOURNAL/st126` §8 ghi mức VS3 | R23 §6 · ST-126 · JOURNAL/st126-dong-bang-api.md §8 |
| 114 | `JOURNAL/st126` §8 ghi mức VS3 | R23 §6 · ST-126 · JOURNAL/st126-dong-bang-api.md §8 |
| 115 | `JOURNAL/st127` §8 ghi mức VS3 | R23 §6 · ST-128 · JOURNAL/st127-semver.md §8 |
| 116 | `JOURNAL/st128` §8 ghi mức VS3 | R23 §6 · ST-128 · JOURNAL/st128-resolver.md §8 |
| 118 | `R23` §6 ghi “(VS3)”; `JOURNAL/st128` §8 mức VS3 | R23 §6 · ST-128/129 · JOURNAL/st128-resolver.md §8 |
| 119 | `JOURNAL/st129` §9 ghi mức VS3 | R23 §6 · ST-129 · JOURNAL/st129-lock.md §9 |
| 120 | VD-133: nợ #115 (số cũ) chuyển VS3 | R23 §6 · ST-130 · VD-133 |
| 127 | `JOURNAL/st132` §7 ghi VS3 | R23 §6 · ST-132 · JOURNAL/st132-landlock.md §7 |
| 137 | VD-145: sửa cần libc/`unsafe` ⇒ VS3; VS2 giữ bước xác nhận + giới hạn trong `HOST-BAO-DAM.md` | R24 §5 · ST-137 · VD-145 |
| 143 | VD-154(b): điều kiện VD-133 đã thoả (**ca `st130b` xanh** từ đợt 23) ⇒ `NamePolicy::Strict` chỉ bật cho tệp **trong cây module** là việc VS3 | JOURNAL/st130-siet-caps.md §9 · VD-133 · VD-149 · VD-154(b) |

## 6. Nợ “không làm có lý do” (1 mục)

| Số | Mô tả | Lý do không làm | Nguồn |
|---|---|---|---|
| 104 | LSP chưa publish chẩn đoán tệp phụ cho tài liệu chính | **VD-126**: LSP chỉ publish chẩn đoán cho tài liệu client đang mở; lỗi module phụ báo qua vị trí import. Publish cho URI chưa mở gây hiển thị không nhất quán giữa các client | R22 §5 · VD-126 · R23 §4 hàng 5 |

Ghi chú: các mục **có chủ đích** trong báo cáo nhưng **chưa có văn bản đóng** vẫn giữ `mở`, chờ Agent 0:
#3 (gán lồng sâu `VS0412` có chủ đích), #27 (LSP EOF ⇒ exit 0 khác spec, cần cho test/CI).

## 7. Số trùng nghĩa — tách bằng hậu tố (không đổi số gốc)

### 7.1 `#109` — hai nghĩa, đã tách trong bảng chính

| Số trong sổ | Nghĩa | Nguồn | Trạng thái |
|---|---|---|---|
| **#109a** | `next_block()` trả `801` khi khối `07xx` còn chỗ (ST-128) | `JOURNAL/st128-resolver.md` §8 · `QUEUE` ST-128 | chuyển VS3 (trùng nội dung **#116**) |
| **#109b** | **Đường GHI** vẫn kiểm-rồi-mở ⇒ cần dòng giới hạn trong `HOST-BAO-DAM.md` | `R23` §6 · `R24` §5 #140 | mở (giới hạn còn nguyên; dòng giới hạn đã có từ `69112900`) |

**Nguồn mâu thuẫn**: `R23 §6` (#109 = đường ghi) **vs** `JOURNAL/st128-resolver.md §8` + `QUEUE ST-128` (#109 = next_block).
Sổ ghi **cả hai**, không chọn một; `#116` (số R23 chuẩn hoá) giữ nguyên nghĩa next_block ⇒ trùng nội dung `#109a`.

### 7.2 Các số từng mang ≥2 nghĩa (đợt 23: ba agent cùng bắt đầu từ 105)

`R23 §6` ghi: “Ba agent làm song song trong đợt 23 đánh số nợ trùng nhau (mỗi người bắt đầu từ 105).
R23 chuẩn hoá lại một dãy duy nhất theo thứ tự hợp nhất.” Bảng ánh xạ số cũ → số trong sổ:

| Số cũ | Nghĩa (nguồn) | Số trong sổ |
|---|---|---|
| 105 | `HSym` builtin `Span::new(0,0)` (`JOURNAL/st122-span-tep.md` §8) | #105 |
| 105 | `TEN_THE` không ràng buộc (`JOURNAL/st126-dong-bang-api.md` §8) | #113 |
| 105 | `--sandbox` chưa cấp `EXECUTE` cho `/lib*` (`JOURNAL/st132-landlock.md` §7) | #125 |
| 106 | `RecDef`/`FieldDef` prelude `file=0` (`JOURNAL/st122` §8) | #106 |
| 106 | hash nguồn VBC1 ≠ khoá cache (`JOURNAL/st126` §8) | #114 |
| 106 | `unicode-ident` ngoài allowlist (`JOURNAL/st132` §7) | #126 |
| 107 | `test/sem` không chứa được ca đa tệp (`JOURNAL/st122` §8) | #107 |
| 107 | `#![allow(dead_code)]` của `semver.rs` (`JOURNAL/st127-semver.md` §8) | đóng ở ST-129 |
| 107 | `ap_dung` mới dùng `ABI::V1` (`JOURNAL/st132` §7) | #127 |
| 108 | quy ước “điền 0” khi so phiên bản (`JOURNAL/st127` §8) | #115 |
| 108 | `pipeline.rs` bám tiền tố grep (`JOURNAL/st131-linux.md` §5) | #121 |
| 109 | `next_block()` trả 801 (`JOURNAL/st128-resolver.md` §8) | #109a / #116 |
| 109 | `sandbox.rs:59` chưa `cfg` theo nền (`JOURNAL/st131` §5) | #122 |
| 110 | `@1.2.3` = ghim (`JOURNAL/st128` §8) | #117 |
| 110 | runner thiếu `wasm32` (`JOURNAL/st131` §5) | #123 |
| 111 | yanked/registry mạng/chữ ký (`JOURNAL/st128` §8) | #118 |
| 111 | `vish_cmd_body.rs:312` đường dẫn thư mục tạm (`JOURNAL/st131` §5) | #124 |
| 112 | `Command::new(tên)` phân giải `PATH` lần nữa (`JOURNAL/st123-va-grant.md` §8) | #108 |
| 112 | lock không ghi quan hệ phụ thuộc (`JOURNAL/st129-lock.md` §9) | #119 |
| 113 | VM không đưa `argv` vào tham số `main` (`JOURNAL/st124-review.md` §7) | #110 |
| 113 | `pkg update` ghim mọi gói khác (`JOURNAL/st129` §9) | **#113b → #142** (VD-149; §7.3) |
| 114 | tyck chấp nhận `let e: cap[Dir] = json.decode(…)` (`JOURNAL/st124` §7) | #111 |
| 114 | `NamePolicy::Strict` bật cho mọi tệp có `use` (`JOURNAL/st130-siet-caps.md` §9) | **#114b → #143** (VD-149; §7.3) |
| 115 | đường GHI kiểm-rồi-mở (`JOURNAL/st124` §7) | #109b |
| 115 | `vis test`/host suite chưa áp strict-caps (`JOURNAL/st130` §9) | #120 |
| 116 | hai bộ escape JSON (`JOURNAL/st125-fuzz-json-esc.md` §7) | #112 |

**Nguồn mâu thuẫn (ghi cả hai, không chọn một):**

- `VD-133` nhắc **#114**/**#115** theo nghĩa **cũ** (`NamePolicy::Strict` / `vis test`–host suite) **vs** `R23 §6`
  (#114 = hash nguồn VBC1, #115 = quy ước điền 0). Sổ giữ số của `R23 §6`; nghĩa cũ ghi ở bảng trên.
- `QUEUE ST-128` ghi “nợ #109” = `next_block` ⇒ **#109a**; `QUEUE ST-124` ghi “nợ #115” = đường ghi ⇒ **#109b**.
- `QUEUE ST-125` ghi “nợ #116” = hai bộ escape JSON ⇒ **#112** (R23 dùng #116 cho `next_block`).

### 7.3 Hai nợ của đợt 23 **không** được R23 gộp vào dãy chuẩn — nay đã có số **142/143** (VD-149)

| Nhãn tạm | Nghĩa | Nguồn | Số chuẩn + trạng thái |
|---|---|---|---|
| **#113b** | `pkg update <tên>` ghim mọi gói khác (`=<version>`): gói được update cần nâng dep chung thì resolver báo `VS0704` thay vì nâng; cần quyết định chính sách | `JOURNAL/st129-lock.md` §9 (số cũ 113) · `QUEUE` ST-129 | **#142** (VD-149) — bảng chính: **mở** (VS2 **không** sửa; xét ở VS3 nếu cần) |
| **#114b** | `NamePolicy::Strict` bật cho **mọi** tệp có `use` (`modules.rs:606`), không chỉ tệp trong cây module; từng che lỗi VD-124 | `JOURNAL/st130-siet-caps.md` §9 (số cũ 114) · `VD-133` · `JOURNAL/st130b-ca-thu-8.md` | **#143** (VD-149) — bảng chính: **chuyển VS3** từ ST-154 (**VD-154(b)**; trước đó **mở**); VD-133 cho phép chuyển VS3 khi ca khoá xanh (ca `st130b` **đã xanh**) |

**Ánh xạ chính thức (VD-149, 2026-11-17): `#113b → #142`, `#114b → #143`.**
Nhãn `#113b`/`#114b` là **số cũ + hậu tố**, **không** phải số của dãy chuẩn: giữ lại ở đây làm ánh xạ,
**không** đánh lại số cũ, **không** tái dùng số **142/143** cho nghĩa khác. Báo cáo R26 trở đi trích bằng
số mới, **kèm số cũ trong ngoặc** — ví dụ `#142` (`#113b`).

### 7.4 Ca 7 self-test của ST-147 **không phải nợ có số** — **đóng ở ST-153** (VD-152, ghi chú)

| Mục | Nội dung |
|---|---|
| Triệu chứng | psvn: self-test case âm **11/12** — ca **7** `doc-examples: ngoại lệ cũ đã xanh`; 6 ca encoding khác đã xanh nhờ **VD-150** (ST-147) |
| Nguyên nhân | `crates/vis-cli/src/cli/check.rs` tách tên gốc bằng `rsplit('/')`; trên Windows (`\`) `phu` **luôn** khác rỗng ⇒ bộ lọc `--syntax` (QD §7.4) **giữ** chẩn đoán **kiểu** ⇒ khối `x = 1` đỏ ⇒ câu `ngoại lệ cũ: …` không bao giờ in (`JOURNAL/st147-chan-doan-psvn.log`) |
| Sửa | **ST-153** (VD-152): MỘT hàm `ten_goc` tách tên gốc theo **cả** `/` và `\`, dùng đúng chỗ cũ; `phu`/`--syntax` **không** đổi gì khác; **+3 ca unit** chạy mọi nền |
| Bằng chứng | psvn @ `4a805115`: self-test **12/12** + cổng **đi QUA bước 5** (dừng bước 10 `L0`, `cap_proc` 150/4 — VS3) — `JOURNAL/st153-gate-psvn.log`; `vis check --syntax C:\…\a.vis` ⇒ `ok 1` `RC=0` (trước ST-153: `not ok … VS0301`) — `JOURNAL/st153-psvn-ca7.log` |
| Trạng thái | **đóng ở ST-153** — **không** cấp số nợ mới: không phải nợ VS3 (lỗi sản phẩm cross-platform đã sửa, không phải giới hạn có lý do) |

## 8. Tóm tắt

| Chỉ số | Số |
|---|---|
| Tổng dòng bảng chính | **147** (1..146; #109 tách `a`/`b`) |
| `mở` | **89** |
| `đóng` | **34** |
| `chuyển VS3` | **21** |
| `không làm có lý do` | **1** |
| Không phải nợ (điều hướng `#141`) | **1** |
| Ngoài dải chuẩn (nhãn tạm `#113b`, `#114b`) | **0** — đã nhận số **#142/#143** (VD-149, §7.3) |
| Mục **không truy được nguồn** (không đưa vào sổ) | **0** |

Kiểm: 90 mở + 34 đóng + 21 VS3 + 1 không làm + 1 điều hướng = **147** dòng. (Dòng **#146** thêm bởi **VD-158** — ca đua symlink **CHƯA-PHÂN-LOẠI**, điều tra ST-163.)

**Vì sao 146 chứ không phải 144 — đối chiếu VD-154(b) (ghi lại thay vì giấu):** mức sổ **trước** ST-154 là **144 dòng** =
91 mở / 31 đóng / 20 VS3 / 1 không làm / 1 điều hướng (ST-151). ST-154 làm **ba** việc trên sổ: (1) `#143` **mở → chuyển VS3**
(VD-154(b)) ⇒ mở −1, VS3 +1 → **90 mở / 31 đóng / 21 VS3 = 144 dòng**, đúng **nguyên văn** con số của VD-154(b); (2) thêm dòng
**#144** (`VD-153`, **đóng ở ST-154**) ⇒ đóng +1; (3) thêm dòng **#145** (`VD-154(c)`, **mở**) ⇒ mở +1. Luật sổ (§3: "chỉ thêm
dòng mới ở cuối bảng, không xoá dòng"; VD-147(3): mỗi nợ **một số, một dòng**) ⇒ tổng thành **146 dòng**. Vậy `90/31/21` là số
**đúng** cho bảng 144 dòng sau bước (1), còn số **cuối cùng** của sổ sau ST-154 là `91/32/21/1/1 = 146`; hàng QUEUE ST-154 ghi
`90 mở/31 đóng/21 VS3` là số của bước (1), cần Agent 0 đính chính nếu muốn R27 trích số cuối. Máy kiểm:
`JOURNAL/st154-kiem-trung.log`.

**Đóng trong đợt 24g (ST-148 đối chiếu lại, chỉ ghi khi có bằng chứng trong repo):** `#89` (ST-139 · PR #24 ·
`ef6489d1` · `JOURNAL/st139-do-truoc.log`), `#130` (ST-142 · `JOURNAL/st142-ps-*.log`: 447/447 hai đường,
`lech=0`), `#139` (ST-140 · `JOURNAL/st140-red-ps.log` + `st140-cargo-test-ps.log`), `#126` (ST-132 · VD-135),
`#131` (ST-00 · đính chính R23/R24 §1). Giữ **mở**: `#109b`/`#140` (đường ghi — dòng giới hạn đã có trong
`HOST-BAO-DAM.md`; sửa kỹ thuật thuộc VS3).

**Đóng trong đợt 25:** `#136` (ST-149 · nợ phân loại thân `cmd` — biến bind từ ngoài che tên lệnh;
đỏ-trước `JOURNAL/st149-do-truoc.log`, tái hiện `JOURNAL/st149-tai-hien.log`, `cargo test --workspace` 1163/0),
`#133` (ST-150 · ca âm **ghi** ở tiến trình con cho thư mục **chỉ-đọc**: `exit=7` + `Permission denied`/`EACCES`,
tệp không được tạo, đối chứng dương ghi được; `ps` 14/14 xanh — `JOURNAL/st150-landlock-ps.log`),
`#135` (ST-151 · danh sách sync của `bin/vis-linux` thành **một nguồn** + `--check-sync` + **9 ca** khoá: bỏ một mục ⇒ đỏ;
`JOURNAL/st151-do-truoc.log` + `JOURNAL/st151-gate-macos-lan2.log`),
`#144` (ST-154 · ca 15 `con_read_dir_thu_muc_co_quyen_ghi_van_liet_ke_duoc`: **dương** `read_dir` trên thư mục **có quyền ghi**
⇒ `exit 0` + `co.txt`, đối chứng ghi `exit 0`, âm ngoài phạm vi `exit 7` + `Permission denied`; `ps` **15/15** xanh —
`JOURNAL/st154-landlock-ps.log`).

**Chuyển VS3 trong đợt 25:** `#143` (ST-154 · **VD-154(b)**: ca khoá `st130b` **đã xanh** ⇒ điều kiện VD-133 đã thoả).
**Thêm số trong đợt 25:** `#144` (VD-153), `#145` (VD-154(c) — chú thích `rt.rs` nói sai đường chặn `chan(0)`, **mở**).

**Đóng trong đợt 28 (ST-157):** `#110` — VM đưa `argv` (`list[str]`) vào tham số **không-`cap`** của `main` như interp
(`interp.rs:221-234`; ST-72b giữ `Value::Cap` đúng **chỉ số**), ca đỏ-trước **3/4 đỏ** (interp `[a, b]` vs VM `()`,
`JOURNAL/st157-do-truoc.log`), sau sửa **4/4 xanh** + `cap_khong_lo::doi_so_dong_lenh_la_str` (ca **ghim** sự lệch cũ)
cập nhật để khẳng định **parity** thay vì ghim lệch. Sổ **146 mục = 90 mở / 33 đóng / 21 VS3 / 1 không làm / 1 điều hướng**.

## 9. Máy kiểm của task này

**ST-145 (lập sổ)**:

- Trích nguồn: `JOURNAL/st145-trich-no.log` (script `/tmp/st145-trich-no.py`, sha256 ghi trong log).
- Kiểm trùng số trong bảng chính: `JOURNAL/st145-kiem-trung.log` ⇒ **0 số trùng** (142 dòng).
- `python3 test/report/check_report.py --all` ⇒ PASS (24 báo cáo, 164 hàng QUEUE).
- `cd viscript && ../vis-dev gate --fast` ⇒ PASS (L0 1159/0, report-check 24/164, doc-examples 24 khối) —
  log `JOURNAL/st145-gate-macos.log` (header VD-136 + sha256 + `[exit=0]`).

**ST-148 (cập nhật theo VD-149, HEAD `b855caa3`)**:

- Thêm số: **#142** (`#113b`), **#143** (`#114b`); ánh xạ ở §7.3 — hai số **không** tái dùng số cũ.
- Kiểm trùng số + dải + nguồn + ánh xạ + khớp §8: `JOURNAL/st148-kiem-trung.log` (script `/tmp/st148-kiem-trung.py`,
  sha256 trong log) ⇒ **PASS, 0 số trùng**, dải **1..143** đủ, 144 dòng, trạng thái **94 mở / 28 đóng / 20 VS3 /
  1 không làm / 1 điều hướng**.
- `python3 test/report/check_report.py --all` ⇒ **PASS (25 báo cáo, 170 hàng QUEUE)** — `JOURNAL/st148-check-report.log`.
- `cd viscript && ../vis-dev gate --fast` ⇒ **PASS**, `[exit=0]`, **10/10 bước OK**, L0 **1159/0** trên 129 suite,
  report-check PASS (25/170), doc-examples OK (24 khối) — `JOURNAL/st148-gate-macos.log` (header VD-136 + sha256
  + `[exit=0]`).

**ST-149 (nợ #136, HEAD `456e8a4a` + mã/ca ST-149)**:

- Đỏ-trước: `cargo test -p vis-cli --test vish_cmd_body -- --test-threads=1` ⇒ ca
  `bien_bind_cap_tep_trung_ten_lenh_la_bieu_thuc` in **bảng `ls`** thay vì `3` (19/20 xanh) —
  `JOURNAL/st149-do-truoc.log` (sha256 `63928844f57e640f5185f8b61f9c59041c626b7447d92a1abd72cee6807f8068`).
- Sửa: `crates/vis-types/src/lower/khai.rs` **+37/−6** (helper `ten_bien_bind_toi` + `la_lenh_da_biet` loại trừ biến
  bind từ ngoài); **4 ca mới** (20 ca) trong `crates/vis-cli/tests/vish_cmd_body.rs`; tái hiện in **`3`**
  (`JOURNAL/st149-tai-hien.log`, sha256 `6beba7334be163a1e3a3c1eb0245236bd86b4263dea091599508819561273d8c`).
- `cargo test --workspace` ⇒ **1163/0** (129 suite; 1159 + 4 ca) — `JOURNAL/st149-cargo-test.log`
  (sha256 `96ab2705eedf4450d41e6108d7b6430b71bcd5b75bf76fc9e64330fdf47e2237`).
- `cd viscript && ../vis-dev gate --fast` ⇒ **PASS**, `[exit=0]`, **10/10** bước `--- …: OK`, L0 **1163/0** (129 suite),
  report-check PASS (25 báo cáo, 171 hàng QUEUE), doc-examples OK (24 khối) — `JOURNAL/st149-gate-macos.log`
  (header VD-136 + sha256 + `[exit=0]`, sha256 `ad574c8e1994a0e48e62b2214e957dd0eb74fa504a5ef06878b2803451005c2f`).

**ST-150 (nợ #133, HEAD `8abb0c81` + ca ST-150 chưa commit, đo trên `ps`)**:

- Ca 14 `con_ghi_vao_thu_muc_chi_doc_bi_kernel_chan` trong `vis-host-linux/tests/landlock.rs` (**+101** dòng, ca cũ
  không sửa): đỏ-trước `exit=0` + nội dung `truoc ruleset`; sau ruleset (2 luật trong **một** `ap`) tiến trình con
  `ghi` vào thư mục chỉ-đọc ⇒ `exit=7` + `loi: Permission denied (os error 13)` (`EACCES`), `x.txt` **không** được tạo;
  đối chứng dương cùng ca `ghi` vào thư mục `ghi: true` ⇒ `exit=0` + `duoc ghi`.
- `ps`: **14/14 xanh** (13 ca cũ vẫn xanh), `test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;
  finished in 0.33s` — `JOURNAL/st150-landlock-ps.log` (sha256 `614b124a…`, 4 699 byte, header VD-136).
- sha256 tệp đo **hai phía khớp**: local `fa2aab0fd1bfd122fb11eee1783adee50639e935b8dde324b6b64d37deaa1461` =
  `ssh root@ps sha256sum /root/vifast-linux/vis-host-linux/tests/landlock.rs` (cùng giá trị); HEAD `8abb0c81` hai phía.
- `cd viscript && ../vis-dev gate --fast` ⇒ **PASS**, `[exit=0]`, **10/10** bước, L0 **1163/0** (129 suite),
  report-check PASS (25 báo cáo, 171 hàng QUEUE), doc-examples OK (24 khối) — `JOURNAL/st150-gate-macos.log`
  (header VD-136 + sha256 + `[exit=0]`, sha256 `6aa8b4f8…`). `vis-host-linux` **không** thuộc workspace `viscript/`.
- Sau khi sửa sổ (`#133` đóng): đếm lại **144 dòng** = **92 mở / 30 đóng / 20 VS3 / 1 không làm / 1 điều hướng**;
  `python3 test/report/check_report.py --all` ⇒ **PASS (25 báo cáo, 171 hàng QUEUE)** — `JOURNAL/st150-check-report.log`.

**ST-151 (nợ #135, HEAD `32b79d4a` + mã/ca ST-151 chưa commit)**:

- `bin/vis-linux` **+44/−10**: `SYNC_DIRS="viscript docs/viscript vis-host-linux vis-host-wasm .git"` +
  `SYNC_FILES="AGENTS.md vis-dev vis"` ở đầu tệp (sau `set -u`); `sync_tree()` chỉ đọc hai biến (bỏ 5 chỗ rải trong vòng lặp
  + `rsync` `AGENTS.md` + vòng `vis-dev`/`vis`). **Không đổi hành vi**: 8 lời gọi `rsync` của bản cũ và bản mới trên cùng một
  cây giả (stub `rsync`/`ssh`) **giống hệt** (thứ tự, `--delete`, exclude, đích); `AGENTS.md` vẫn best-effort
  (`2>/dev/null || true`), hai shim vẫn `die 3` — `JOURNAL/st151-do-truoc.log` §[0].
- `--check-sync`: in 8/8 mục `có|thiếu`; `exit=0` khi đủ, `exit=2` khi thiếu (`thiếu n/8`); **không** ssh/rsync
  (ca tĩnh khoá `cmd_check_sync` không chứa `ssh`/`rsync`; nhánh `--check-sync` không gọi `pick_backend`).
- **9 ca** `crates/vis-cli/tests/sync_list.rs` (khuôn `soak_script.rs`): đỏ-trước `error: no test target named sync_list`
  (exit **101**) ⇒ xanh 9/9; **bỏ `vis-host-wasm` khỏi `SYNC_DIRS`** ⇒ **4 ca đỏ**; **rải lại `for d in AGENTS.md vis-dev vis`**
  ⇒ ca `sync_tree_chi_dung_hai_bien` đỏ; hoàn nguyên sha256 `aedf781a…` — `JOURNAL/st151-do-truoc.log`
  (sha256 `2bf3b11655627b6b7eeb6bf6d33f394445fd7eb1d824c17eee87b9f4946fd30f`).
- `macOS` gate lần 1 **ĐỎ** ở L0 — ca **THỜI GIAN** `lsp::module::tests::hieu_nang_nap_module_5_tep` (`116 ms ≥ 100 ms`
  dưới tải `7.39/6.18/5.27`, 12 core) ⇒ **T-INFRA** (D2-07), chạy riêng **5/5 xanh** (`JOURNAL/st151-lsp-hieu-nang.log`);
  log đỏ giữ nguyên `JOURNAL/st151-gate-macos.log` (sha256 `8626ab3b…`). Lần 2 **PASS** `[exit=0]`, **10/10** bước,
  L0 **1175/0** (**130** suite; = 1166 + 9 ca), report-check PASS (25 báo cáo, 171 hàng QUEUE), doc-examples OK (24 khối)
  — `JOURNAL/st151-gate-macos-lan2.log` (sha256 `a53c38a829727829256a783982487f297f24aecf52208a518f6bbb6959aedd2e`).
- `--check-sync` trên macOS: `exit=0`, 8/8 `có` — `JOURNAL/st151-check-sync-macos.log` (sha256 `935fb81f8927fee495ba780bcb64c84def36e5e5971853ff6ee12eb10d2bcc0d`).
- Sau khi sửa sổ (`#135` đóng): đếm lại **144 dòng** = **91 mở / 31 đóng / 20 VS3 / 1 không làm / 1 điều hướng**;
  trùng số 0, dải **1..143**.

**ST-154 (nợ #144 + sổ theo VD-154, HEAD `0582913b` + ca 15/SO-NO/log chưa commit)**:

- Ca **15** `con_read_dir_thu_muc_co_quyen_ghi_van_liet_ke_duoc` trong `vis-host-linux/tests/landlock.rs`
  (**+137** dòng, **không** sửa ca nào cũ, không `#[ignore]`/`cfg`-bỏ): `co.txt` ghi **trước** khi áp ruleset; **một** ruleset
  **hai** luật (`trong` `ghi:false` + `trong-ghi` `ghi:true`, `kq.so_luat == 2` như ca 14) · **dương**: helper `ls trong-ghi`
  ⇒ **`exit 0`** + danh sách `co.txt`; **đối chứng luật ghi có hiệu lực**: helper `ghi trong-ghi/moi.txt` ⇒ `exit 0`, `ls` lại
  thấy **`co.txt` + `moi.txt`** · **âm cùng ca**: helper `ls ngoai` ⇒ **`exit 7`** + `loi: Permission denied (os error 13)`
  (`EACCES`) · đỏ-trước in `exit=0` + `co.txt`; helper gọi thẳng `std::fs::read_dir`, **không** qua `FsGuard` ⇒ **kernel** chặn.
- `ps`: **15/15 xanh** (`test result: ok. 15 passed; 0 failed; 0 ignored`, 0,35 s; 14 ca cũ vẫn xanh) — kernel
  `7.0.0-43-generic`, Ubuntu 26.04.1, rustc 1.99.1, `lsm` có `landlock` — `JOURNAL/st154-landlock-ps.log`
  (5 278 byte, sha256 `cb4ab4ab1b57f5d653b19addd21cced7dab877a74eb9557876ecb7865b95dbca`, header VD-136 + `### EXIT_VIS_LINUX_RUN=0`).
- **sha256 tệp đo HAI PHÍA khớp**: local `7102e90b3a484a9128fe981240090e7ccd43a7e9f96c6c05f4576ac57e871627` =
  `ssh root@ps sha256sum` (cùng giá trị), 25 454 byte. Lần đo **1** (trước `cargo fmt`) giữ nguyên ở
  `JOURNAL/st154-landlock-ps-lan1-truoc-fmt.log` — 15/15 xanh, chỉ khác khoảng trắng trong **một** `assert!` của ca 15.
- Sổ theo **VD-154**: `#143` → **chuyển VS3** (ghi rõ `VD-154(b): chuyển VS3 vì ca st130b xanh`, **giữ** dòng lịch sử);
  `#144` **đóng ở ST-154** (vế 2 của `#133`); thêm **`#145`** (**mở**, xử khi có task VS3 kênh). Sổ **146 dòng** =
  **91 mở / 32 đóng / 21 VS3 / 1 không làm / 1 điều hướng**; trùng số **0**, dải **1..145** — `JOURNAL/st154-kiem-trung.log`
  (PASS 10/10 mục kiểm; sha256 của chính log này ghi ở hàng QUEUE ST-154 + JOURNAL §ST-154 — không tự trích trong tệp đang được băm).
- `90 mở/31 đóng/21 VS3` (VD-154(b), lặp ở hàng QUEUE ST-154) là số của bảng **144 dòng** sau bước đổi `#143` và **trước**
  khi thêm hai dòng `#144`/`#145`; sổ cuối cùng **146 dòng = 91/32/21/1/1** — đối chiếu ở §8 + `st154-kiem-trung.log`.
- `cd viscript && ../vis-dev gate --fast` ⇒ **PASS**, `[exit=0]`, **10/10** bước `--- …: OK`, L0 **1175/0** (**130** suite),
  report-check PASS (**26** báo cáo, **173** hàng QUEUE), doc-examples OK (24 khối) — `JOURNAL/st154-gate-macos-lan2.log`
  (76 725 byte, sha256 `aae5447557dc20f71b8ebfcf9eea31b42bf74f538d957ed98941e0d04ae9053b`, header VD-136 + `[exit=0]`).
  **Lần 1 ĐỎ** ở bước đầu `--- fmt: FAIL` (rustfmt đòi xuống dòng **một** `assert!` của ca 15) — không phải `T-INFRA`:
  sửa fmt rồi chạy **lần 2**, log đỏ giữ nguyên `JOURNAL/st154-gate-macos.log` (2 449 byte, sha256
  `3b0a12649934c5df71b5f5e2bd6c4d264b8d193cb3d541ae117c94704cea2429`).
- `python3 viscript/test/report/check_report.py --all` ⇒ **PASS (26 báo cáo, 173 hàng QUEUE)** — `JOURNAL/st154-check-report.log`.


**ST-157 (đóng #110, HEAD `567350e1`)**:

- `cargo test -p vis-cli` ⇒ **xanh**: 77 suite, **778 / 0** (gồm ca mới `vm_main_argv` **4/4**).
- Ca đỏ-trước: `cargo test -p vis-cli --test vm_main_argv` **3/4 ĐỎ** (interp `[a, b]` vs `--vm` `()`) — `JOURNAL/st157-do-truoc.log`
  (`[exit=101]`), chạy **trước** khi sửa.
- `cd viscript && ../vis-dev gate --fast` ⇒ **PASS** `[exit=0]`, **10/10** bước `--- …: OK`, L0 **1179/0** (**131** dòng
  `test result`, cơ sở 1175 + 4 ca mới) — `JOURNAL/st157-gate-macos.log` (header VD-136 + sha256 + `[exit=0]`).
- `target/debug/vis test test/sem` ⇒ **447/447**, exit 0 — `JOURNAL/st157-vis-test.log`; `--vm` cũng **447/447**
  (0 `not ok`) — `JOURNAL/st157-vis-test-vm.log`.
- Sổ: `#110` **mở → đóng ở ST-157**; §8 **146 = 90/33/21/1/1**; §9 khối này.
