# KT-140 — hai chỗ tài liệu mập mờ + luật đỏ `E_AMBIGUOUS_FIELD`

**Ngày**: 2026-11-18 · **Nguồn**: Agent 0 (phiếu giao qua người điều phối) · **Phạm vi**: hai tài
liệu K6 + `v2/tools/check-scope-table.py` + log ca âm persist.

**Vì sao có task này:** `check-scope-table.py` (KT-134) kiểm **máy** bảng "đối chiếu số ↔ khối
`scope=`" và đang xanh 19/19 ca. Nhưng xanh **không** có nghĩa tài liệu **rõ**: checker chọn "khối
đầu tiên có trường" nên một dòng bảng có thể đúng với khối nó resolve mà vẫn **mâu thuẫn với khối
đầu tiên người đọc tra**. Agent 0 chỉ ra hai chỗ như vậy và yêu cầu sửa cả hai, cộng một luật đỏ
mới để checker bắt được lớp lỗi ấy về sau.

## 1. `k6-exec.md` — trường `observed` có **hai** giá trị ở **hai** khối

Hiện trạng: dòng bảng chỉ ghi tệp log `kt74-qemu-ktest-exec.log`, nhưng tệp ấy được trích ở **hai**
khối in `observed`:

| Khối | Dòng | Giá trị | Tình huống |
|---|---|---|---|
| §2.3 | `# tls: set=0x74310001 observed_first=0 observed=0x74310001 …` | `0x74310001` | lượt chạy **thứ hai** của chương trình A **sau** `SetTls` |
| §7 | `# tls reset: after_exec=0x0 observed=0 …` | `0` | sau `exec` sang ảnh **D** (D **không** gọi `SetTls`) |

Checker cũ resolve sang §7 (khối **đầu tiên có giá trị thập phân** — §2.3 chỉ có hex) nên vẫn xanh,
nhưng người đọc tra khối `observed` **đầu tiên** thấy `0x74310001` và tưởng bảng sai. **Không sửa
nội dung hai khối log** — chỉ sửa bảng + thêm ghi chú.

Dòng bảng sau khi sửa (nguyên văn):

```text
| `observed` | không | `scope=` | `kt74-qemu-ktest-exec.log` | §7 |
```

Ghi chú thêm ngay dưới bảng nói rõ §2.3 là **trường hợp khác** (`0x74310001`, sau `SetTls`), §7 là
khối bảng trỏ tới (`0`, sau `exec`), và vì sao cột "Khối" là thứ phân biệt hai tình huống.

## 2. `k6-persist.md` — khẳng định của ca âm **không** có khối nào trích

Câu ở §3.1: "… ⇒ `match` bằng không và `mismatched` khác không". Đã kiểm: **không** tệp nào trong
`HEAD` chứa dòng `action=verify.*match=0` (`git grep` trên `HEAD -- v2/` rỗng). Khối
`scope-red=kt75-red-selftest.log` ở §7 chỉ có stdout của **runner** (`# red-selftest: PASS …`),
**không** có dòng `# persist:` của kernel — nên khẳng định đúng là **lời khai không bằng chứng**.

**Chạy lại được.** Đúng một lệnh QEMU được phép:

```bash
bash v2/test/boot/persist.sh --red-selftest
```

- 2026-11-18, bắt đầu `04:53:21Z`, exit **0**, `# persist.sh: PASS`.
- Log QEMU boot đọc lại (`b02.log`) in `match=0` và `mismatched=510` — đúng khẳng định.
- Log chép vào repo: [`kt140-persist-red/`](kt140-persist-red/) (kèm `README.md` ghi lệnh + ngày +
  danh mục tệp; **không** chép `b02.host.log` cũ 07:51 vì nó không thuộc lần chạy này).
- Khối mới trong `k6-persist.md` §7 (nguyên văn, `diff` với `b02.log:69-70` **trùng**):

```text
# persist: phase=1 action=verify files=1 bytes=512 read=512 absent=0 match=0 sha_prefix=799edf40 mismatched=510 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
not ok 1 - persist verify phase 1: byte đọc lại LỆCH / băm lệch / nhật ký pha hỏng
```

**Không thêm dòng bảng trùng cho `match`/`mismatched`** (khác gợi ý "nếu cần" của phiếu) — hai lý do
kỹ thuật, không phải lười:

1. Cột "Giá trị (chữ)" chỉ nhận **chữ số tiếng Việt** trong `WORDS`; `mismatched=510` không diễn đạt
   được ("năm trăm mười" không có trong bảng), nên dòng trùng sẽ **không** kiểm được.
2. Dòng trùng `match` = không (0) sẽ làm **văn xuôi** §3.1 ("`match` bằng một") đỏ `E_VALUE_MISMATCH`,
   vì vế văn xuôi so **từng** dòng bảng cùng tên trường. Nới vế đó thành "khớp **một** dòng bất kỳ"
   là **nới lỏng** phép kiểm — đúng thứ Agent 0 cấm.

Thay vào đó: dòng bảng `match`/`mismatched` giữ khối **§3.1** (bảy pha dương) và thêm ghi chú dưới
bảng nói rõ ca âm ở §7 in **cùng** trường với giá trị **khác** — cột "Khối" phân biệt hai tình huống
bằng máy đọc được.

## 3. Luật đỏ mới `E_AMBIGUOUS_FIELD`

**Định dạng "chỉ rõ khối" đã chọn:** thêm cột thứ năm **"Khối"** vào bảng, ghi **số hiệu tiết** dạng
`§7` / `§2.3` (không phải số dòng, không phải văn xuôi). Checker:

1. Đọc **tiêu đề cột** (không theo vị trí): bốn cột đầu phải đúng `Trường đo | Giá trị (chữ) | Loại
   khối | Tệp log`; cột thứ năm, nếu có, phải tên **"Khối"**. Sai ⇒ `E_TABLE_HEADER`.
2. Dựng bản đồ **khối → tiết** từ tiêu đề Markdown **có đánh số** (`## 7. …`, `### 2.3 …`). Tiêu đề
   **không** đánh số (vd `## Bảng đối chiếu …`) đặt lại tiết = `None` — nếu không, khối cuối tài liệu
   sẽ bị gán nhầm vào tiết đánh số cuối cùng và cột "Khối" sẽ "đúng" giả tạo.
3. Với **mỗi** dòng bảng: đếm **số giá trị khác nhau** của trường trong **tất cả** khối
   `scope=`/`scope-red=` — **cả thập phân lẫn hex** (`0x10` và `16` là **một** giá trị). Nhiều hơn
   một **và** ô "Khối" trống ⇒ **`E_AMBIGUOUS_FIELD`**.
4. Ô "Khối" phải đúng dạng `§<số>` và tiết đó phải **có thật** ⇒ nếu không: `E_SECTION_UNKNOWN`.
5. Khối được trỏ tới phải **thật sự** nằm trong tiết đã ghi ⇒ nếu không: `E_SECTION_MISMATCH`. Vế
   này có hiệu lực **cả khi** trường chỉ có một giá trị — cột "Khối" không được là lời khai.
6. Hình dạng: dòng bảng phải đủ số cột như tiêu đề ⇒ `E_TABLE_ROW_SHAPE`.

**Vì sao đếm cả hex** (điểm dễ "tối giản" sai nhất): nếu chỉ đếm thập phân thì `observed` trông như
**một** giá trị (`0`), luật bỏ sót **đúng** ca Agent 0 vừa bắt — tức luật không bắt được lỗi mà nó
sinh ra để bắt. Có ca selftest riêng cho việc này (`0x10` vs `1`).

**Vì sao lọc khối theo tiết** (không chỉ kiểm rồi bỏ qua): nếu vẫn chọn "khối đầu tiên có trường" thì
cột "Khối" chỉ là trang trí — dòng bảng ghi §7 mà resolve sang §2.3 vẫn xanh. Lọc theo tiết làm cột
"Khối" có hiệu lực thật.

**Selftest: 19 → 31 ca** (12 ca mới: ba ca (a)/(b)/(c) Agent 0 yêu cầu + khối sai tiết + tiết không
có + ô sai dạng + một-giá-trị-mà-trỏ-nhầm-tiết + một-giá-trị-trỏ-đúng + hex-vs-thập-phân (âm/dương) +
thiếu cột + tiêu đề sai tên cột). Ca hồi quy hex cũ được **viết lại** cho hai khối **cùng tiết** để
vẫn kiểm "khối hex không che khối thập phân" sau khi thêm bộ lọc tiết.

```text
# check-scope-table-selftest: PASS (31/31 ca)
```

**Kiểm ngược trên tài liệu thật** (bản copy, bỏ `§7` khỏi dòng `observed`):

```text
not ok - v2/docs/k6-exec.md: E_AMBIGUOUS_FIELD: `observed` có 2 giá trị khác nhau trong tài liệu (0, 1949368321 (0x74310001)) nhưng dòng bảng không ghi cột 'Khối' — người đọc tra khối đầu tiên sẽ thấy giá trị khác
# check-scope-table: FAIL (1 lỗi)
```

## 4. Mốc số câu bỏ qua — **KHÔNG đổi**

Trước và sau khi sửa hai tài liệu: **4/19** câu (`k6-exec.md` 1, `k6-persist.md` 3, `K6-DOD.md` 0).
Vì vậy **không** cập nhật `scope-table-skip-count.txt` (checker cũng không ghi lại: số bằng mốc).
Không thêm từ khoá nào vào `QUALIFIERS`; không nới ngưỡng nào.

```text
# check-scope-table: bỏ qua 4/19 câu khẳng định (từ khoá loại trừ) — v2/docs/k6-exec.md 1, v2/docs/k6-persist.md 3, v2/agent/reports/K6-DOD.md 0
# check-scope-table: số câu bỏ qua 4 (mốc 4) — không tăng
# check-scope-table: PASS
```

## 5. Chỗ phiếu mô tả **chưa đúng** hiện trạng

1. **"Checker hiện resolve sang khối §7 nên vẫn xanh"** — đúng, nhưng lý do sâu hơn: §2.3 chỉ có
   **hex** nên `field_value` (chỉ nhận thập phân) bỏ qua khối ấy. Nếu §2.3 in `observed=0` thập phân
   thì checker cũ **đã** đỏ từ trước. Nói cách khác: xanh cũ là **may**, không phải thiết kế.
2. **"thêm dòng cho `match`/`mismatched` trỏ vào khối mới"** — **không làm được** mà không nới phép
   kiểm (lý do ở §2). Đã thay bằng cột "Khối" + ghi chú; báo lại để Agent 0 quyết nếu muốn khác.
3. **"khoảng 30 giây"** — lần chạy này ~10 s cho hai boot (13:53:21 → 13:53:31), chưa kể build
   (cargo dùng cache). Không ảnh hưởng kết luận.
4. **"19 ca tự kiểm"** — đúng (19 trước, 31 sau).
5. **"Log của ca âm nằm ở `/tmp`, không trong repo"** — đúng, và **kiểm được**: `git grep` trên
   `HEAD -- v2/` cho `action=verify.*match=0` **rỗng**. Khối `scope-red=kt75-red-selftest.log` hiện
   có chỉ là stdout **runner**, không có dòng `# persist:` của kernel.

## 6. Tệp đã chạm

| Tệp | Thay đổi |
|---|---|
| `v2/docs/k6-exec.md` | bảng + cột "Khối"; ghi chú `observed` (§2.3 là ca khác) |
| `v2/docs/k6-persist.md` | khối `scope-red=kt140-persist-red/b02.log` ở §7; dòng §3.1 trỏ khối; bảng + cột "Khối"; ghi chú `match`/`mismatched` |
| `v2/agent/reports/K6-DOD.md` | bảng + cột "Khối" (không dòng nào mập mờ, nhưng giữ định dạng thống nhất) |
| `v2/tools/check-scope-table.py` | luật `E_AMBIGUOUS_FIELD` + `E_SECTION_*` + `E_TABLE_*`; đọc bảng theo **tên cột**; 12 ca selftest mới |
| `v2/agent/JOURNAL/kt140-persist-red/` | log ca âm chép vào repo + `README.md` |

**Không** commit, **không** push (theo yêu cầu). **Không** sửa nội dung khối log có sẵn. **Không**
chạy QEMU nào ngoài đúng một lệnh `persist.sh --red-selftest`.
