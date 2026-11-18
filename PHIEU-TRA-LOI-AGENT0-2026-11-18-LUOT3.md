# PHIẾU TRẢ LỜI AGENT 0 — lượt 3 (câu 2, câu 3, Q4 ca biên, luật 2)

**Ngày**: 2026-11-18 · **HEAD**: `80601274`, đã push, cây sạch, `v2 check` PASS, `gate --fast` PASS.

> **Theo luật 1 của anh**: mọi con số dưới đây dán **nguyên văn** từ output công cụ, kèm **lệnh đã
> chạy**. Bản gốc đầy đủ ở [`v2/agent/JOURNAL/kt141-bang-chung/bang-chung.txt`](v2/agent/JOURNAL/kt141-bang-chung/bang-chung.txt).

---

# 1. Câu 2 — nối ba script tự kiểm mồ côi, bật `--standalone-red` vĩnh viễn

## 1.1 Phân loại (thời gian đo thật bằng `date +%s%N`)

| script | thời gian đo (ms) | vào cổng nào | vì sao |
|---|---|---|---|
| `v2/test/rules/selfbreak.sh` | 94.878 · 105.275 | `gate --fast` | chép cả cây sáu lần rồi chạy `verify-numbers.py` năm luật trên từng bản |
| `v2/tools/git-guard/neg-test.sh` | 12.219 | `gate --fast` | có **ghi** lên repo (`git worktree add`, nhánh tạm) ⇒ không chen vào `check` |
| `v2/tools/git-guard/neg-test-kt85b.sh` | 26.320 | `gate --fast` | `git clone --local` cả repo vào `/tmp` + cài hook + PROBE trên repo thật |

Cả ba đều chậm hơn ngưỡng năm giây ⇒ cả ba vào `gate --fast`.

## 1.2 `verify-numbers.py --self-test` nay được cổng chạm tới — **có thật**

`selfbreak.sh` **thật sự gọi** (không phải chỉ nhắc trong chú thích). Bằng chứng thực thi từ `bash -x`:

```text
663:+ /opt/homebrew/bin/python3.12 /var/folders/.../kt80-selfbreak.96zZQ5/case-5/v2/tools/verify-numbers.py --root /var/folders/.../case-5 --self-test
```

Và ca thứ năm chỉ xanh nếu `--self-test` chạy thật: `ok 5 - luat 8: verifier bị làm yếu ⇒ self-test ĐỎ (E8_CASE_GREEN)`.

## 1.3 Sau khi nối

```text
$ python3 v2/tools/check-selftest-wiring.py --standalone-red | tail -1
check-selftest-wiring: PASS (11 script tự kiểm: 11 có cổng, 0 ngoại lệ, 0 mồ côi)
```

**Không ngoại lệ nào.** Cả mười script lớp độc lập đều `CÓ`.

## 1.4 Đột biến — đỏ đúng tên script

Gỡ dòng gọi `selfbreak.sh`:

```text
MỒ CÔI (ĐỎ)
- E_ORPHAN v2/test/rules/selfbreak.sh: script tự kiểm độc lập, không cổng nào gọi (đã quét v2/bin/v2 + v2/test/rules/ (9 tệp))
...
check-selftest-wiring: FAIL (1 script tự kiểm không cổng nào gọi: v2/test/rules/selfbreak.sh)
```

Khôi phục: sha256 `79b7c294…90bb7` trùng đúng mốc trước đột biến.

## 1.5 Năm hàng rào phải báo (không thêm ngoại lệ nào)

- **R1** — `gate --fast` đang đỏ ở bước có **từ trước** (`check-scope-table.py` báo `E_AMBIGUOUS_FIELD`). Đã chứng minh nó đỏ **cả trên worktree sạch ở HEAD** ⇒ không do việc này. Nay đã sửa (câu 3).
- **R2** — `selfbreak.sh` đỏ trong working tree vì task song song sửa tài liệu trích log **chưa `git add`**; `copy_repo` cố ý chỉ chép tệp đã index. Chạy trên worktree HEAD sạch: **5/5 PASS**. Script lành; đỏ là hàng rào liên-task.
- **R3** — `neg-test.sh` lần đầu đỏ vì mười bốn mục worktree rác; chính script `prune` chúng, chạy lại PASS.
- **R4** — hai script git-guard cần hook đã cài (`install.sh`); bản clone mới sẽ đỏ kèm thông điệp rõ.
- **R5** — `neg-test.sh` dùng tên nhánh cố định `task/kt85neg` ⇒ hai cổng chạy song song sẽ đụng.

---

# 2. Câu 3 — hai chỗ tài liệu, và luật đỏ `E_AMBIGUOUS_FIELD`

## 2.1 `k6-exec.md` — dòng `observed` nay trỏ đích danh khối

Dòng bảng sau khi sửa:

```text
| `observed` | không | `scope=` | `kt74-qemu-ktest-exec.log` | §7 |
```

Ghi chú ngay dưới bảng nói rõ: khối **§7** là `observed` bằng **không** (sau `exec` sang ảnh D,
ảnh D không gọi `SetTls`); khối **§2.3** cũng in trường `observed` nhưng là **trường hợp khác**
(`0x74310001` — lượt chạy thứ hai của chương trình A **sau** `SetTls`), **không** dùng để đọc dòng
bảng này.

## 2.2 `k6-persist.md` — ca âm **chạy lại được**, log đã vào repo

```text
$ bash v2/test/boot/persist.sh --red-selftest
# persist: phase=1 action=verify files=1 bytes=512 read=512 absent=0 match=0 sha_prefix=799edf40 mismatched=510 size_bad=0 absent_bad=0 recorded=1 pending_blocks=0 replayed=0
not ok 1 - persist verify phase 1: byte đọc lại LỆCH / băm lệch / nhật ký pha hỏng
```

Log chép vào [`v2/agent/JOURNAL/kt140-persist-red/`](v2/agent/JOURNAL/kt140-persist-red/), khối
`scope-red=` đặt ở §7 của tài liệu, dòng §3.1 trỏ tới khối đó.

**Một chỗ tôi không làm được như anh gợi ý**: thêm **dòng bảng** cho `match`/`mismatched`. Cột
"Giá trị (chữ)" chỉ nhận chữ số tiếng Việt nên không diễn đạt được `mismatched=510`; và một dòng
`match` = không sẽ làm văn xuôi "`match` bằng một" đỏ oan. Nới vế đó thành "khớp một dòng bất kỳ"
là **nới phép kiểm** — tôi tránh. Cần anh quyết nếu muốn khác.

## 2.3 `E_AMBIGUOUS_FIELD` — và nó bắt được **nhiều hơn** chỗ anh chỉ ra

Thiết kế: thêm cột thứ năm **"Khối"** ghi số hiệu tiết (`§7`, `§2.3`). Checker đếm **số giá trị
khác nhau** của trường trong **tất cả** khối, **cả hex** (`0x10` ≡ `16`); nhiều hơn một giá trị mà
ô "Khối" trống ⇒ **ĐỎ**. Có "Khối" thì **lọc khối theo tiết** trước khi resolve — nếu không, cột
"Khối" chỉ là trang trí.

Đếm cả hex là điểm cốt lõi: chỉ đếm thập phân thì `observed` trông như **một** giá trị và luật bỏ
sót đúng ca anh vừa bắt.

**Kết quả khi bật luật**: ngoài `observed`, nó bắt thêm **bốn** trường mập mờ khác mà cả anh và tôi
đều chưa biết — `same` (`k6-exec.md`: `same=1` ở §4.3 vs `same=0` ở khối đỏ §5), `ok`, `device_flushes`
(bảy giá trị), `absent` (`k6-persist.md`). Đã ghi tiết cho tất cả.

```text
$ python3 v2/tools/check-scope-table.py --selftest | tail -1
# check-scope-table-selftest: PASS (31/31 ca)
```

## 2.4 Số câu bỏ qua — **không đổi**

Trước **4/19**, sau **4/19**. Không thêm từ khoá nào vào `QUALIFIERS`. Vì số không đổi nên **không**
cập nhật mốc; lý do ghi ở [`kt140-journal.md`](v2/agent/JOURNAL/kt140-journal.md).

## 2.5 Một đính chính về mô tả của tôi

Tôi nói "checker resolve sang §7 nên vẫn xanh". Đúng, nhưng lý do sâu hơn: §2.3 chỉ có **hex** nên
`field_value` (chỉ nhận thập phân) **bỏ qua** khối ấy. Nếu §2.3 in `observed` bằng số thập phân thì
checker cũ **đã** đỏ từ trước. **Xanh cũ là may, không phải thiết kế.**

---

# 3. Q4 — năm ca biên mới, phán quyết tách từng lệnh

## 3.1 Năm ca mới, hành vi **thật** đọc từ mã

| lệnh | hành vi thật | khẳng định đã viết |
|---|---|---|
| `time` có tham số | chỉ khớp **nguyên dòng** ⇒ in dấu hỏi | dấu hỏi là dòng kết quả đầu; không dòng `time ns=` |
| `write` có đường dẫn, thiếu nội dung | **cắt tệp về rỗng**, in `bytes` bằng rỗng | đúng một dòng `bytes` rỗng **và** `stat` thấy `size` rỗng |
| `write` vào thư mục không có | `lookup` trượt ⇒ `create` trượt ⇒ lỗi "không tìm thấy" | đúng một dòng lỗi FS "không tìm thấy" |
| `ls` lên một tệp | `kind` khác `INODE_DIR` ⇒ lỗi "không phải thư mục", trả **trước** khi in `ls n=` | đúng một dòng lỗi FS "không phải thư mục"; không dòng `ls n=` |
| `sync` có tham số | như `time` có tham số ⇒ in dấu hỏi | dấu hỏi là dòng kết quả cuối; không dòng `sync ok` |

## 3.2 Một chỗ hành vi **lệch** yêu cầu của anh — cần anh quyết

Anh yêu cầu: "`write` có đường dẫn nhưng thiếu nội dung ⇒ **phải báo lỗi rõ ràng**, không được ghi
rỗng". **Hành vi thật ngược lại**: shell **không báo lỗi**, nó **cắt tệp về rỗng**.

Tôi ghim **hành vi thật**, không ghim kỳ vọng — luật repo: code là nguồn sự thật số một. Nếu anh
muốn đổi thành lỗi thì phải thêm nhánh `text.is_empty()` trong `cmd_write`/`fs_dispatch`; khi đó ca
`new-cmds-edge3[write-thieu-noi-dung]` sẽ **đỏ** và phải đổi theo. **Đây là quyết định của anh.**

## 3.3 Phán quyết tách từng lệnh

Trước: một dòng PASS/FAIL cho năm lệnh, chỉ in lý do lệnh hỏng **đầu tiên**. Nay: chạy **hết** mọi
lệnh rồi mới kết luận, in **một dòng cho mỗi lệnh**, và khi hỏng in lý do của **mọi** lệnh hỏng.
Áp cho cả bốn ca. Đã chứng minh bằng đột biến: sửa hàm quay lại `return 1` ngay lỗi đầu ⇒ **5/32
kiểm đỏ, exit 1**.

## 3.4 Bằng chứng chạy thật

```text
$ bash v2/test/boot/shell-edge.sh --selftest | tail -1
# shell-edge-selftest: PASS (8/8 ca, 32/32 kiểm: 18 sai→ĐỎ + 14 đúng→XANH)
```

Trên **ảnh sản phẩm** `af01e670…`:

```text
# new-cmds-edge3[time-tham-so]: PASS
# new-cmds-edge3[write-thieu-noi-dung]: PASS
# new-cmds-edge3[write-thu-muc-khong-co]: PASS
# new-cmds-edge3[ls-tep]: PASS
# new-cmds-edge3[sync-tham-so]: PASS
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 + new-cmds-edge3 PASS
# shell-edge.sh: PASS
```

## 3.5 Còn tám chỗ mỏng (trung thực)

`cat` lên thư mục · `write` lên thư mục · `truncate` lên thư mục · `stat` lên thư mục · `ls` đường
dẫn tương đối · `write` với đường dẫn rỗng nhưng có dấu cách · `time` hai tham số · `truncate` thiếu
kích thước. **Không chỗ nào chặn K6.**

---

# 4. Luật 2 — quét lại **toàn bộ** phạm vi build-inputs

Nhật ký đầy đủ: [`v2/agent/JOURNAL/kt134-scope-rescan.md`](v2/agent/JOURNAL/kt134-scope-rescan.md).

## 4.1 Quét ra **hai** lỗi nữa, không chỉ một

**(a) Ba mươi ba tệp test tích hợp nằm trong phạm vi.** `v2/kernel` + `v2/lib` có ba mươi ba tệp
trong `tests/` của mười ba thư mục. Cargo **không bao giờ** biên dịch chúng vào binary phát hành.
Đã loại `*/tests`, `*/benches`, `*/examples`.

**(b) `meta` làm việc loại `tests/` vô hiệu.** `vbrev_inputs_calc` gộp ba nguồn: `tracked` (từng
tệp), `pins` (năm tệp), `meta` (thư mục gói cargo). `meta` được băm bằng `git rev-parse HEAD:<thư-mục>`
— trả **tree oid**, mà tree oid gộp **mọi** tệp bên trong, kể cả `tests/`. Nên chỉ cần `meta` có mặt
là việc loại `*/tests/*` **không có tác dụng**. Ca tự kiểm (j) đỏ đúng vì lý do này trước khi sửa:

```text
# vbrev-selftest (j) FAIL: sửa tệp tests/ mà dấu vân tay ĐỔI (9ea4df40781645e1 -> a9c52950ada6d4ce) — test tích hợp còn trong phạm vi
```

Đã sửa: chỉ giữ `meta` cho thư mục **nằm ngoài** các gốc (đó mới là mục đích ban đầu của nó).

## 4.2 Cố ý **không** loại `src/`

Trong `src/` có ba tệp trông như test. **Giữ nguyên**, vì một tệp trong `src/` **có thể** được `mod`
không-test biên dịch — không phân biệt được bằng đường dẫn; và `ktest.rs` là nguồn của **binary ảnh
test**, mà ảnh test cũng đo bằng cùng dấu vân tay này.

Nguyên tắc: **thừa còn hơn thiếu**. Thừa ⇒ phải build lại (phiền). Thiếu ⇒ hai ảnh khác nhau cùng
một dấu vân tay (nguy hiểm).

## 4.3 Kết quả

```text
$ bash v2/test/boot/build-rev.sh --selftest | tail -1
# vbrev-selftest: PASS (a/b/c/d/e/f/g/h/i/j)
```

Đột biến chứng minh ca (j) không phải xanh rỗng — bỏ dòng loại `*/tests/*`:

```text
# vbrev-selftest (j) FAIL: sửa tệp tests/ mà dấu vân tay ĐỔI (26191c6838c097a5 -> 55d8b3811e76e352) — test tích hợp còn trong phạm vi
```

Đổi định nghĩa ⇒ bump `VBREV_ALGO` `kt134-v2` → `kt134-v3`, cập nhật ghim, rebuild.

## 4.4 Việc còn lại của luật 2

- **Danh sách ghim** — đã quét: năm tệp, cả năm đều là đầu vào thật. Không có tệp nào để loại.
- **Từ khoá loại trừ** (`QUALIFIERS`) — **chưa** quét lại toàn bộ. Ghi ở đây để không quên.

---

# 5. Trạng thái

```text
$ ./v2/bin/v2 status
== dòng OS ViFastOs ==
HEAD        3b132dcd75d0 (main)  chưa push: 0
build-inputs 210 mục  dấu vân tay: 1324edbf33f315b8  dirty: 0
ảnh mới nhất af01e670275b4104954816ae537ad63bc0ee807bba7496dcb8872044683315b8.bin
             sha256=af01e670275b4104954816ae537ad63bc0ee807bba7496dcb8872044683315b8 size=205440
             # build: inputs=1324edbf33f315b8 dirty=0
task        DONE=129 READY=3 BLOCKED=0 REVIEW=0
```

Sáu bộ tự kiểm, nguyên văn:

```text
$ bash v2/test/boot/shell-edge.sh --selftest | tail -1
# shell-edge-selftest: PASS (8/8 ca, 32/32 kiểm: 18 sai→ĐỎ + 14 đúng→XANH)
$ python3 v2/tools/check-scope-table.py --selftest | tail -1
# check-scope-table-selftest: PASS (31/31 ca)
$ python3 v2/tools/check-selftest-wiring.py --standalone-red | tail -1
check-selftest-wiring: PASS (11 script tự kiểm: 11 có cổng, 0 ngoại lệ, 0 mồ côi)
$ bash v2/test/boot/build-rev.sh --selftest | tail -1
# vbrev-selftest: PASS (a/b/c/d/e/f/g/h/i/j)
$ bash v2/tools/check-algo-pin.sh --selftest | tail -1
# algo-pin-selftest: PASS (12 ca: a/b/c/d/e/f/g/h/i/j/k)
$ bash v2/test/rules/counter-evidence.sh --selftest | tail -1
# counter-evidence-selftest: PASS (4/4 ca)
```

---

# 6. Việc kế tiếp, và **ba câu hỏi**

Theo thứ tự anh chốt: **KT-136** (soak đĩa thật) → **nhúng `VBREV_ALGO` vào log/ảnh** → **KT-81** →
**K7**.

**Ba câu hỏi cần anh trả lời:**

1. **`write` có đường dẫn nhưng thiếu nội dung** — giữ hành vi hiện tại (cắt tệp về rỗng) hay đổi
   thành **báo lỗi**? Nếu đổi, tôi phải sửa `cmd_write`/`fs_dispatch` và ca kiểm sẽ đỏ trước. (§3.2)
2. **Dòng bảng cho `match`/`mismatched`** — anh gợi ý thêm dòng bảng, nhưng làm vậy phải **nới** vế
   văn xuôi của checker. Tôi đã tránh. Anh muốn giữ cách hiện tại (khối `scope-red=` + ghi chú), hay
   chấp nhận nới? (§2.2)
3. **Từ khoá loại trừ `QUALIFIERS`** — luật 2 nói phải quét lại toàn bộ phạm vi. Việc này chưa làm.
   Anh muốn tôi quét ngay (thêm một lượt), hay gộp vào task sau?
