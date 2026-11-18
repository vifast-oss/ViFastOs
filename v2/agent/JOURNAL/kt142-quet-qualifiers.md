# KT-142 — quét toàn bộ `QUALIFIERS` của `check-scope-table.py`

**Việc Agent 0 giao (phiếu lượt 4, câu 3, nguyên văn):**

> ## Câu 3 · QUALIFIERS = QUÉT NGAY
> Với từng từ khoá: liệt kê mọi câu nó đang loại, đánh dấu loại đúng / loại oan.
> Từ khoá nào loại oan ít nhất 1 câu khẳng định thật ⇒ thu hẹp hoặc bỏ, câu đó phải qua checker.
> Ghi kết quả vào kt134-scope-rescan.md (hoặc tệp riêng). Số câu bỏ qua đổi ⇒ cập nhật mốc kèm lý do.

Tệp này là "tệp riêng" ấy. Log nguyên văn: `kt142-logs/kt142-quet-truoc.log` (bản cũ) và
`kt142-logs/kt142-quet-sau.log` (bản mới).

## 1. Cách quét

Mỗi từ khoá trong `QUALIFIERS` được đối chiếu với **mọi** câu mà nó đang loại, trên **cả ba** tài
liệu có bảng đối chiếu (`v2/docs/k6-exec.md`, `v2/docs/k6-persist.md`,
`v2/agent/reports/K6-DOD.md`). Cách làm: nạp chính mô-đun checker rồi gọi `check_doc()` cho từng
tài liệu, gom `Skip` theo `keyword`. Không đọc bằng mắt, không suy từ tên từ khoá — lý do: bản
thân phép quét phải tái lập được, và một từ khoá "nghe hợp lý" vẫn có thể đang nuốt câu thật.

## 2. Bảng quét — 14 từ khoá, 4 câu bị loại

| Từ khoá | Số câu đang loại | Câu (tệp:dòng) | Loại đúng / oan |
|---|---|---|---|
| `nhánh` | 1 | `k6-persist.md:78` | **đúng** — câu nói nhánh `durable` bằng không **chưa có ca**, tức mô tả *thiếu* bằng chứng, không khẳng định số đo |
| `ca âm` | 1 | `k6-persist.md:76` | **đúng** — câu khẳng định `match` bằng không của **khối đỏ** `kt140-persist-red/b02.log`, không phải khối xanh dòng bảng trỏ tới |
| `ca đỏ` | 0 | — | không loại câu nào |
| `chưa có` | 0 | — | không loại câu nào |
| `nếu` | 0 | — | không loại câu nào |
| `đột biến` | 0 | — | không loại câu nào |
| `mong` | 0 | — | không loại câu nào |
| `phải là` | 0 | — | không loại câu nào |
| `sẽ` | 0 | — | không loại câu nào |
| `vắng` | 1 | `k6-persist.md:188` | **đúng** — câu khẳng định `absent` bằng một của **khối đỏ** `kt75-qemu-b10.log` |
| `bỏ ` | 0 | — | không loại câu nào |
| `tăng gấp đôi` | 0 | — | không loại câu nào |
| `khác không` | 0 | — | không loại câu nào |
| `đỏ` | 1 | `k6-exec.md:154` | **đúng** — câu khẳng định `same` bằng không của **khối đỏ** `kt74-red-qemu.log` |

**Kết luận vế "loại oan":** không từ khoá nào loại oan một câu khẳng định **về khối mà dòng bảng
trỏ tới**. Cả bốn câu bị loại đều nói về một khối **khác** hoặc về một ca **chưa có**.

Nhưng đó chưa phải toàn bộ sự thật, và đây là chỗ Agent 0 bắt đúng: **ba trong bốn câu ấy là
khẳng định thật, có số thật, và checker hoàn toàn kiểm được** — nếu nó so với khối mà *câu* nói
tới thay vì khối mà *dòng bảng* trỏ tới. Bộ từ khoá đang gánh một việc mà phép kiểm làm được.
Đó là lỗ thật, chỉ khác lỗ mà Agent 0 mô tả ở chỗ: nó không nuốt câu **sai**, nó nuốt câu **đúng**
— nghĩa là nó làm phép kiểm yếu đi mà không ai thấy.

## 3. Bốn câu ấy, nguyên văn

**`k6-persist.md:76`** — khẳng định `match` bằng không, và câu **ghi rõ tên khối**:

```text
| `match`, `mismatched` | **có ca âm thật**: bộ kiểm đỏ-trước/tự phá sửa payload trên đĩa ⇒ `match` bằng không và `mismatched` khác không — **khối `scope-red=kt140-persist-red/b02.log` ở §7** (KT-140: trước đó khẳng định này **không** có khối nào trích trong repo) |
```

**`k6-persist.md:188`** — khẳng định `absent` bằng một, nằm trong tiết 6.2 (tiết chỉ có khối đỏ):

```text
boot đọc lại thấy `/pneg.txt` **vắng** (`absent` bằng một) trong khi bảy pha trước vẫn
```

**`k6-exec.md:154`** — khẳng định `same` bằng không, câu **ghi rõ loại khối** `scope-red`:

```text
`scope-red` dưới), TAP `exec giu pid cspace` **đỏ** (`same` bằng không).
```

**`k6-persist.md:78`** — câu duy nhất **không** phải khẳng định số đo (nói nhánh bằng không *chưa
có ca*):

```text
nhánh `durable` bằng không chưa có ca |
```

## 4. Sửa gì

Thêm `resolve_claim_block()`: câu văn xuôi được so với **khối mà câu nói tới**, không phải khối
của dòng bảng. Bốn luật, **chặt trước, lỏng sau**, mỗi luật chỉ nhận khi kết quả **duy nhất**:

1. Câu ghi đúng `scope=<tệp>` / `scope-red=<tệp>` ⇒ khối đó.
2. Câu ghi **loại** khối (`scope-red` / `scope`) mà không ghi tệp ⇒ khối **cùng tiết** cùng loại.
3. Câu nằm trong tiết có khối chứa trường ⇒ khối cùng tiết.
4. Không quy được ⇒ dùng khối của dòng bảng (hành vi cũ, không đổi).

"Duy nhất" xét theo **giá trị**, không theo số khối: tiết 3.1 có nhiều khối cùng ghi `recorded=1`
— chúng không mâu thuẫn nên vẫn quy được. Chỉ khi các khối **nói khác nhau** mới bỏ qua bước đó,
vì lúc ấy không có căn cứ chọn.

Nhờ vậy **năm** từ khoá không còn việc: `nhánh`, `ca âm`, `ca đỏ`, `vắng`, `đỏ`, `khác không` —
đã **bỏ**. (`khác không` lộ ra ở lần chạy đầu: sau khi bỏ `ca âm`, câu `k6-persist.md:76` bị `khác
không` bắt, vì câu có mệnh đề "`mismatched` khác không". Nhưng `khác không` **không bao giờ** sinh
ra một khẳng định "bằng <số>" — regex chỉ khớp `bằng`, nên từ khoá ấy chỉ có tác dụng nuốt khẳng
định của **trường khác** trong cùng câu. Đó đúng là loại oan, nên bỏ.)

Còn lại **8** từ khoá, tất cả thuộc nhóm "câu không phải số đo": `chưa có`, `nếu`, `đột biến`,
`mong`, `phải là`, `sẽ`, `bỏ `, `tăng gấp đôi`.

## 5. Kết quả đo

Trước (bản `HEAD`, log `kt142-logs/kt142-quet-truoc.log`):

```text
TONG cau bi bo qua: 4
So tu khoa dang dung: 4 / 14
```

Sau (log `kt142-logs/kt142-quet-sau.log`):

```text
TONG cau bi bo qua: 1
So tu khoa dang dung: 1 / 8
```

Câu duy nhất còn bị bỏ qua là `k6-persist.md:78` (`chưa có`) — đúng, vì đó không phải khẳng định
số đo.

**Ba câu trước kia được miễn nay được so thật**, và đều **xanh** — nghĩa là ba khẳng định ấy đúng.
Nếu chúng sai, cổng sẽ đỏ ngay lần chạy này.

## 6. Đột biến (luật 3 vế của Agent 0)

`--selftest` từ 31 lên **39 ca**. Tám ca mới cho KT-142, mỗi luật có **ca dương và ca âm** (ca
dương một mình không chứng minh gì: một hàm luôn trả `None` cũng làm mọi ca dương xanh).

Ba đột biến đã chạy, cả ba đều bị bắt:

| Đột biến | Ca đỏ | Kết quả |
|---|---|---|
| `resolve_claim_block` trả `None` luôn (tắt quy khối theo câu) | 7/8 ca KT-142 | `FAIL (32/39 ca)` |
| `_unique_value` trả `values[0]` (bỏ bước "phải duy nhất") | ca "hai khối nói khác nhau" | `FAIL (38/39 ca)` |
| bỏ luật 1 (câu ghi tên khối) | 2 ca "ghi tên khối" | `FAIL (37/39 ca)` |

Ca "hai khối nói khác nhau" chỉ bắt được đột biến thứ hai sau khi **đảo thứ tự** hai khối trong
fixture (khối đỏ đứng trước khối xanh). Lý do: đột biến trả giá trị khối **đầu**; nếu khối đầu
trùng giá trị dòng bảng thì đột biến vẫn xanh oan. Đã ghi chú ngay trong fixture.

## 7. Mốc số câu bỏ qua

`v2/agent/JOURNAL/scope-table-skip-count.txt` ghi lại **1** (trước là **4**), kèm lý do: số **giảm**
vì năm từ khoá đã bỏ và ba câu nay được so thật — không phải vì nới ngưỡng. Mốc giảm thì checker
**không** cảnh báo (chỉ cảnh báo khi tăng), nên lần chạy sau vẫn bắt được nếu ai thêm từ khoá mới.
