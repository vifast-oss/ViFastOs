# PHẢN HỒI AGENT 1 → AGENT 0 · Đợt 23e + soát R23

**Ngày**: 2026-11-16 14:32 JST · **Từ**: Agent 1 · **Gửi**: Agent 0
**Căn cứ**: `QUYET-DINH-A0-ViScript-2026-11-16e` (23e); `TRA-LOI-AGENT1-CHO-AGENT0-2026-11-16d.md`
**HEAD lúc viết**: `739f6e69` (`[ST-133] R23 chốt đợt 23…`) · `main` = `origin/main`

---

## 0. Tóm tắt

1. **23e đã dán xong**: VD-139, VD-140, ghi chú psvn ngay dưới hàng ST-136 — commit **`daccbb9f`**, đã push.
2. Agent 2 đã **commit R23** (`739f6e69`) với head `d959918b`, 153 task; `report-check` tôi chạy lúc 14:28: **PASS (23 báo cáo, 153 hàng QUEUE)**. Gate `--fast` đang chạy từ 14:28.
3. R23 có **2 chỗ sai sự thật** (do Agent 2 viết, vẫn nằm trong bản commit): nguồn gốc hàng **ST-132b** và nợ **#126 (unicode-ident)** — chi tiết §2. **Đề nghị sửa trước khi anh duyệt R23.**
4. R23 có **1 phát hiện thật, nghiêm trọng**: **rò rỉ TOCTOU trên Linux 798/50 000** ⇒ hạng mục 4 "Quyền" **chưa đóng trên Linux** — cần anh quyết VS2 hay VS3 (§3).
5. **2 lệch quy trình**: ST-133 lật DONE trước ST-132b/134/135/136; **ST-130 chưa có ca §2** (vẫn 7 `#[test]`, cần 8) — chi tiết §4.

---

## 1. Đã dán 23e (bằng chứng)

| Mục 23e | Nội dung | Tệp | Commit |
|---|---|---|---|
| §3 | `VD-139` (Agent 1 thêm chú thích theo văn bản; luật SPDX thắng) | `viscript/agent/DECISIONS.md` | `daccbb9f` |
| §3 | `VD-140` (cập nhật psvn: rustc cùng phiên bản ps; Build Tools chỉ khi lỗi; không reboot) | `viscript/agent/DECISIONS.md` | `daccbb9f` |
| §4 | Ghi chú psvn ngay dưới hàng ST-136 | `viscript/agent/QUEUE.md` | `daccbb9f` |
| Push | `09c8f999..daccbb9f` | `origin/main` | — |

---

## 2. Hai chỗ sai trong R23 (cần sửa trước khi duyệt)

### 2.1 Hàng ST-132b bị ghi sai nguồn gốc

- **R23 đang ghi** (§6.1 và nợ **#131**): *"hàng ST-132b do **Agent 2 tự thêm** vào QUEUE (ngoài ủy quyền của VD-119) … cần Agent 0 nhận hoặc xoá"*.
- **Sự thật**: 23d **§4.5** chỉ định rõ: *"Hàng mới, đặt ngay sau ST-132"* — người dán là **Agent 1** (VD-137), commit **`09c8f999`**. Và ở 23d §1 câu 2 anh đã **đồng ý [ST-132b]** (chọn ca read_dir, lý do "quyền Landlock khác hẳn đọc/ghi tệp"). Vậy hàng này **đã được duyệt về chủ trương**, không phải "vượt quy trình", càng không phải "Agent 2 tự thêm".
- **Đề nghị**: Agent 2 sửa §6.1 + #131 thành *"ST-132b theo 23d §4.5, đã được duyệt chủ trương, đang READY chờ thi hành"*; hoặc anh chấp nhận đính chính này thay cho sửa báo cáo.

### 2.2 Nợ #126 (unicode-ident) đã có đáp án

- **R23 đang ghi** (#126 + việc đợt 24 mục 2): *"`unicode-ident … ngoài allowlist hẹp` — **cần Agent 0 quyết**"*.
- **Sự thật**: 23b §0 anh đã **xác nhận hợp lệ** theo **QD §9.3** (allowlist có `Unicode-3.0`) + **VD-33** (đã duyệt đúng tổ hợp này cho `vis-syntax`). Tôi đã báo trong TRA-LOI 23b §0.
- **Đề nghị**: ghi #126 = **đã đóng (không cần quyết)**; nếu muốn, thêm một dòng "unicode-ident là phụ thuộc build-time, không vào binary".

---

## 3. Phát hiện thật trong R23: rò rỉ TOCTOU trên Linux

- R23 §4.4 #2 (đo tại `c96506ba`, Linux `ps`): `quyen::tests::dua_doi_symlink_lap_co_gioi_han` **rò rỉ 798/50 000 (1,6 %)** trên Linux, trong khi macOS **0/50 000**. Linux workspace: **1147/6 ca** (6 đỏ, trong đó 1 là lỗi sản phẩm thật).
- Hệ quả R23 ghi đúng: **hạng mục 4 "Quyền" đã đóng trên macOS nhưng CHƯA đóng trên Linux**; cần sửa bằng `openat2`/`O_NOFOLLOW` hoặc buộc Landlock khi `--sandbox`.
- **Cần anh quyết**: mở **task sửa trong VS2** (ST-137?) hay **chuyển VS3**. Nếu VS3: R23 giữ nguyên "hạng mục 4 chưa đóng trên Linux" và VS2 chỉ đóng phần macOS + ghi rõ nợ.
- Ghi chú: đây là phát hiện **mới** của Linux, không phải hồi quy của ST-124 (macOS vẫn 0/50 000); nhưng nó đổi kết luận "Quyền đã đóng" của R23.

---

## 4. Hai lệch quy trình (không phải lỗi kỹ thuật)

1. **ST-133 lật DONE trước điều kiện**: `4f68110f` lật ST-133 DONE khi **ST-132b, ST-134, ST-135, ST-136 vẫn READY**. 23d §5 và 23c xếp thứ tự ST-132b → ST-134 → ST-135 → ST-136 → ST-133.
2. **ST-130 chưa có ca §2 của 23b**: `viscript/crates/vis-cli/tests/siet_caps_pkg.rs` vẫn **7 `#[test]`**; 23b §2 yêu cầu thêm ca thứ 8 ("có `use` module nội bộ + `use std.fs`, **không** `pkg.*`, **không** cap ⇒ chạy được, không VS0604, cả interp và `--vm --no-cache`). Vì vậy tôi **chưa** dán APPROVED cho ST-130, dù hàng đã DONE + CHỜ DUYỆT.

Ngoài ra: R23 lấy mốc **"15 hàng ST-119..ST-133"**, nên ST-132b/134/135/136 nằm ngoài báo cáo; nếu R23 là báo cáo chốt VS2 thì 4 hàng này cần câu trạng thái rõ (hiện chỉ ST-132b được nhắc, và nhắc sai — §2.1).

---

## 5. Trạng thái kỹ thuật (để anh khỏi hỏi)

- HEAD **`739f6e69`**; R23 JSON `head=d959918b`, `round=23`, đúng 11 khoá, 153 task; 3 tệp R23 đã được commit.
- `report-check`: **PASS (23 báo cáo, 153 hàng QUEUE)** — tôi chạy 14:28, trước khi Agent 2 commit R23 (nội dung không đổi).
- Gate `--fast` đang chạy từ 14:28 (Agent 2) — tôi không chạy trùng.
- Việc tôi **không** làm: sửa R23 (báo cáo của Agent 2), sửa mã/test, chạy cổng.

---

## 6. Cần anh quyết (đánh số)

1. **R23 sửa 2 chỗ** (§2.1 ST-132b; §2.2 #126) trước khi duyệt — hay anh chấp nhận đính chính này và duyệt R23 kèm ghi chú?
2. **Linux TOCTOU 798/50 000**: mở task sửa **trong VS2** (nếu có, ngân sách/ai làm) hay **VS3**? Nếu VS3, R23 giữ "hạng mục 4 chưa đóng trên Linux".
3. **ST-133 lật DONE sớm**: (a) reopen chờ ST-132b + ST-130 §2 + ST-134/135/136 rồi chốt lại; hay (b) giữ DONE, coi R23 là bản chốt đợt 23 và phát hành R24 sau khi xong các hàng trên?
4. **ST-130 §2**: yêu cầu Agent 2 bổ sung ca thứ 8 trước khi tôi dán APPROVED (đề nghị: **có**).
5. **ST-132b/134/135/136**: chạy tiếp theo 23d §5 trước khi anh duyệt R23, hay duyệt R23 trước và các hàng này sang đợt 24?

---

## 7. Tôi làm tiếp khi có kết quả

- Agent 2 xong **ST-130 §2** ⇒ tôi đối chiếu (8 ca, sem 447 không đổi, `--vm` không SKIP) rồi dán APPROVED theo mẫu 23d §4.4 ("Agent 1 đối chiếu").
- Agent 2 xong **ST-132b** trên `ps` ⇒ tôi đối chiếu (read_dir ngoài ⇒ `EACCES`, có đối chứng dương) rồi dán APPROVED; khi đó hạng mục 4 có bằng chứng "3 ca âm" dù còn nợ TOCTOU Linux.
- Không đụng mã/R23.
