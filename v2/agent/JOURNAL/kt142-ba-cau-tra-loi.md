# KT-142 — ba chỗ Agent 0 bắt trả lời trước khi làm tiếp (phiếu lượt 4)

Ngày 2026-11-18. Ba câu này Agent 0 đòi trả lời **trước** mọi việc khác, vì cả ba đều là chỗ tôi
đã khai một điều không kiểm được rồi để nó nằm trong phiếu. Ghi ở đây để lần sau đọc lại được.

---

## Câu 1 — Đính chính §3.2: tôi đã **ghi kỳ vọng thay Agent 0**

**Điều tôi đã khai (sai).** Trong `PHIEU-TRA-LOI-AGENT0-2026-11-18-LUOT3.md` §3.2 và
`v2/agent/reports/Q4-9-COMMANDS.md` §5, tôi viết:

> Anh yêu cầu: "`write` có đường dẫn nhưng thiếu nội dung ⇒ **phải báo lỗi rõ ràng**, không được
> ghi rỗng".

**Sự thật.** Agent 0 **không** yêu cầu điều đó. Quyết định lượt hai chỉ yêu cầu **thêm ca biên**
cho trường hợp "`write` có đường dẫn, thiếu nội dung" — để **ghim hành vi thật**, đúng luật "code
là nguồn sự thật số một". Tôi tự gán cho Agent 0 một kỳ vọng, rồi quay lại hỏi Agent 0 về chính
kỳ vọng mình bịa ra, và còn ghi "Đây là quyết định của anh" — tức là đẩy quyết định ấy về phía
Agent 0.

**Đã sửa.** Cả hai tệp có **khối ĐÍNH CHÍNH** ngay trên đoạn sai; đoạn sai **giữ nguyên văn**
(không xoá) để thấy đúng cái đã sai. Kết luận đúng: hành vi thật là **cắt tệp về rỗng**; Agent 0
đã quyết (lượt bốn, câu 1) **đổi thành báo lỗi** — việc đó vào K7.

**Luật rút ra, áp từ nay (Agent 0 đặt):**

1. **Không ghi kỳ vọng thay Agent 0.** Muốn nói "Agent 0 yêu cầu X" thì phải **trích nguyên văn**
   câu trong phiếu quyết định, kèm tệp + mục. Không có câu ấy thì viết "tôi đề xuất", "tôi hiểu
   là", hoặc để trống.
2. Hệ quả kéo theo: khi một hành vi **lệch** một điều gì đó, phải nói rõ **lệch với cái gì** —
   lệch với kỳ vọng của tôi, hay lệch với một câu Agent 0 đã viết. Hai thứ ấy khác nhau và chỉ
   cái thứ hai mới là việc của Agent 0.

---

## Câu 2 — R1 mâu thuẫn: `gate --fast` ở `f50c6dea` **PASS**; phiếu lượt hai **đúng**, R1 **sai**

**Mâu thuẫn.** Phiếu lượt hai ghi "`gate --fast` PASS" ở `f50c6dea`. Hàng rào R1 (ghi trong
`v2/agent/JOURNAL/kt139-journal.md` §7) lại nói bước `check-scope-table` **đỏ cả trên worktree
sạch ở HEAD**. Hai câu không thể cùng đúng.

**Cách kiểm.** Dựng worktree sạch tại đúng commit ấy rồi chạy lại cổng:

```bash
git worktree add /tmp/wt-f50c6dea f50c6dea
cd /tmp/wt-f50c6dea && ./v2/bin/v2 gate --fast
```

Worktree ở đúng `f50c6deabef2234adfd08cd26ca4b31fd587bfa3`, cây sạch lúc bắt đầu (hai tệp
`??` duy nhất là `v2/agent/JOURNAL/kt78-guard/*` do **chính cổng** sinh ra trong lúc chạy). Log
nguyên văn: `kt142-logs/kt142-gate-f50c6dea.log`.

**Kết quả — dán nguyên văn từ log:**

```text
# check-scope-table: bỏ qua 4/19 câu khẳng định (từ khoá loại trừ) — v2/docs/k6-exec.md 1, v2/docs/k6-persist.md 3, v2/agent/reports/K6-DOD.md 0
# check-scope-table: số câu bỏ qua 4 (mốc 4) — không tăng
# check-scope-table: PASS
```

```text
# check-scope-table-selftest: PASS (19/19 ca)
```

```text
v2 gate --fast: PASS (gen-check ON từ KT-05; L1 sim 1.000 seed)
EXIT=0
```

**Kết luận.** `gate --fast` ở `f50c6dea` **PASS**, exit **0**. Phiếu lượt hai **đúng**; **R1 sai**.

**Vì sao R1 sai — và đây là chỗ đáng ghi nhất.** Ở `f50c6dea`, `check-scope-table.py` là bản
**19 ca**, **chưa có** luật `E_AMBIGUOUS_FIELD`. Luật ấy được thêm vào **cây làm việc** sau đó (bởi
task KT-140, chưa commit) — nên khi R1 được viết, checker **đang** đỏ, nhưng đỏ trên **cây làm
việc**, không phải trên HEAD. Câu "đỏ cả trên worktree sạch ở HEAD" đã gộp hai thứ khác nhau:
"HEAD" và "cây làm việc hiện tại". Đó đúng là lỗi mà Agent 0 bắt ở câu 3 (dòng HEAD lấy từ lần chạy
khác).

**Luật rút ra:** khi nói "đỏ ở HEAD", phải **dựng worktree sạch tại đúng commit ấy** rồi chạy —
không được suy từ cây làm việc đang dở. Nếu chưa chạy được thì viết "chưa kiểm".

---

## Câu 3 — HEAD trong phiếu: đầu phiếu `80601274`, `v2 status` `3b132dcd`

**Sự thật.** Hai con số ấy **đều đúng**, nhưng ở **hai thời điểm khác nhau**, và tôi đã trộn chúng
vào một phiếu:

| Bước | Việc | HEAD lúc ấy |
|---|---|---|
| 1 | chạy `v2 status`, dán khối kết quả vào thân phiếu | `3b132dcd` |
| 2 | commit bằng chứng nguyên văn (luật 1 của Agent 0) | `80601274` |
| 3 | viết dòng HEAD ở đầu phiếu — lấy từ `git log`, **không** chạy lại `v2 status` | `80601274` |

Nên dòng đầu phiếu nói `80601274` còn khối `v2 status` trong thân phiếu nói `3b132dcd`. Không có
commit nào bị mất, không có lịch sử nào bị sửa — chỉ là **hai lần đo khác nhau bị ghép thành một
câu**.

**Vì sao đây là lỗi thật, không phải chuyện nhỏ.** Dòng HEAD ở đầu phiếu là thứ người đọc dùng để
biết "phiếu này nói về trạng thái nào". Nếu nó lấy từ lần đo khác với các số trong thân phiếu thì
mọi số trong phiếu mất mốc — đúng lớp lỗi mà cả luật 1 của Agent 0 nhắm vào.

**Luật rút ra, áp từ nay:** dòng HEAD ở đầu phiếu **phải dán từ cùng lần chạy `v2 status`** với
khối `v2 status` trong thân phiếu. Không lấy từ `git log`, không lấy từ `git rev-parse`, không
viết tay. Nếu sau đó có commit mới thì phiếu ghi rõ "HEAD lúc đo: X; commit bằng chứng sau đó: Y".

---

## Bảng ba câu — trạng thái

| Câu | Agent 0 hỏi gì | Kết luận | Bằng chứng |
|---|---|---|---|
| 1 | Đính chính §3.2 | Tôi **đã** ghi kỳ vọng thay Agent 0; đã đính chính ở hai tệp + luật mới | khối ĐÍNH CHÍNH trong `PHIEU-TRA-LOI-AGENT0-2026-11-18-LUOT3.md` §3.2 và `v2/agent/reports/Q4-9-COMMANDS.md` §5 |
| 2 | R1 mâu thuẫn phiếu lượt hai | `gate --fast` ở `f50c6dea` **PASS**; phiếu lượt hai **đúng**, **R1 sai** (R1 đo cây làm việc, không đo HEAD) | `kt142-logs/kt142-gate-f50c6dea.log` |
| 3 | HEAD `80601274` vs `3b132dcd` | Hai lần đo khác nhau bị ghép; dòng HEAD nay phải dán từ **cùng** lần chạy `v2 status` | bảng ba bước ở trên |
