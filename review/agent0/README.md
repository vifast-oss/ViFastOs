<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Gói chờ Agent 0 ký — YC-02..05 (2026-11-12)

**Đây không phải nguồn sự thật và không phải duyệt.** Đây là đầu vào để Agent 0 quyết
trong một lượt, trước khi rời phiên. Duyệt chỉ có giá trị bằng dòng
`APPROVED <YYYY-MM-DD> Agent 0` trong `v2/agent/QUEUE.md` (AGENTS.md §8).

## Vì sao để ở đây

- `docs/` là tài liệu thiết kế/QD đã đóng băng — không quẳng bản nháp vào.
- `v2/agent/` là sổ của Agent 2 — Agent 2 không được tự sửa QUEUE/DECISIONS.
- `spec/` chỉ Agent 0 (và KT-04a) được ghi.
- `review/` đã là workspace review, được track; gói này nằm ở `review/agent0/`, xoá được sau khi ký.

## Agent 0 cần làm (khoảng 10 phút)

1. Đọc [`QUYET-DINH-YC.md`](QUYET-DINH-YC.md) — 5 quyết định, mỗi cái có **đề xuất mặc định**.
2. Ghi 5 mục D2-29..D2-33 vào `v2/agent/DECISIONS.md` (câu chữ mẫu trong tệp trên).
3. Lật 3 ô trạng thái ở `v2/agent/QUEUE.md` (bảng trong `QUYET-DINH-YC.md` mục 6).
4. Nếu đồng ý đề xuất: không cần sửa `spec/` tay — D2-32/D2-33 cho Agent 2 áp đúng draft
   đã có, kèm máy kiểm, một commit xanh.
5. Chạy [`KIEM.sh`](KIEM.sh) sau khi Agent 2 áp (hoặc ngay bây giờ để biết hiện trạng).

## Tùy chọn nếu Agent 0 rời máy

- Mặc định: **không** ủy quyền duyệt. KT-05 chờ Agent 0 quay lại; Agent 2 chuyển sang
  KT-06/KT-08/KT-12/KT-T26/R1 và dòng ViScript (đủ việc nhiều giờ).
- Nếu muốn chạy tiếp qua đêm: đọc [`TUY-CHON-DUYET-VANG-MAT.md`](TUY-CHON-DUYET-VANG-MAT.md),
  ký hoặc không ký D2-34. Không ký thì không ai được ghi APPROVED thay.

## Hiện trạng lúc 18:10

| Việc | Trạng thái |
|---|---|
| KT-04a `spec/abi.toml` lõi + bảng ánh xạ | **DONE** `1766145` |
| KT-04b object reserved | **BLOCKED** YC-04 — draft `v2/docs/abi-reserved-objects.toml` + checker đã có |
| KT-04c 21 `.scn` + checker + 3 fixture | **DONE phần viết** `9552832`; phần arity **BLOCKED** YC-05 |
| KT-04d gửi duyệt | **REVIEW**, chưa gửi (chờ 04b/c) |
| KT-05 gen-check | **BLOCKED** chờ APPROVED |
| Agent 2 hiện tại | đang **KT-T26** (host test `vifast-fs`), không ngồi chờ |

## Việc của Agent 2, không cần chữ ký

- `v2/docs/abi-v1-v2-mapping.md:114-116` còn sai số: ghi `27 method / 24 đích / 19 bỏ`,
  đếm đúng là **26 / 23 / 20**; hàng 17 (`:72`) có `Abort|Defer` chưa escape làm vỡ bảng.
- Sửa trước khi gửi KT-04d, không đụng quyết định nào ở đây.

## Sau khi ký

- Agent 2 áp D2-32 (reserved objects) + D2-33 (draft chữ ký) + D2-29 (DebugGetInfo),
  cập nhật máy kiểm, chạy `gate --fast`, commit `[KT-04b]`/`[KT-04c]`.
- KT-04d gửi lại Agent 0 kèm 21 `.scn` + checklist; **APPROVED mở KT-05**.
- Gói này có thể xoá hoặc giữ làm dấu vết; nếu giữ, không trích dẫn nó như nguồn sự thật.
