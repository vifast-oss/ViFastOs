<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# Q4 — chín lệnh shell: bảng ca đúng / ca biên trên **ảnh sản phẩm**

Người ghi: **Agent 2 dòng OS** · Ngày: **2026-11-18** · HEAD khi làm: `ba8486ec`
· Tệp được sửa: [`v2/test/boot/shell-edge.sh`](../test/boot/shell-edge.sh)

> **Vì sao có báo cáo này.** Agent 0 yêu cầu bảng **đủ chín dòng** cho chín lệnh shell cần có ca kiểm
> trên ảnh sản phẩm. Phiếu trước của lead chỉ nêu **tên bảy lệnh** nên bị trả lại. Bảng dưới đây là
> bản đầy đủ, kèm **số dòng thật** của từng khẳng định (lấy bằng `grep -n`, không chép tay).

---

## 1. Bảng chín dòng

| lệnh | ca **đúng** | ca **biên** | tệp:dòng khẳng định |
|---|---|---|---|
| `ls` | `new-cmds-ok` | `new-cmds-edge` | `v2/test/boot/shell-edge.sh:305` |
| `cat` | `write-cat-bytes`, `write-de-co-lai` | `cat-khong-tham-so`, `cat-khong-co` | `v2/test/boot/shell-edge.sh:453` |
| `write` | `write-cat-bytes`, `write-de-co-lai` | `write-khong-tham-so` | `v2/test/boot/shell-edge.sh:452` |
| `time` | `new-cmds-ok` | `new-cmds-edge` | `v2/test/boot/shell-edge.sh:283` |
| `uptime` | `new-cmds-ok` | `new-cmds-edge2` | `v2/test/boot/shell-edge.sh:290` |
| `sync` | `new-cmds-ok` | `new-cmds-edge` | `v2/test/boot/shell-edge.sh:295` |
| `stat` | `new-cmds-ok`, `truncate-jobs` | `new-cmds-edge` | `v2/test/boot/shell-edge.sh:300` |
| `truncate` | `truncate-jobs` | `new-cmds-edge` | `v2/test/boot/shell-edge.sh:247` |
| `jobs` | `truncate-jobs` | `new-cmds-edge2` | `v2/test/boot/shell-edge.sh:257` |

## 2. Khẳng định của **ca biên** — dòng nào, kiểm cái gì

Cột cuối ở bảng trên chỉ ghi dòng khẳng định **định dạng đầu ra** của lệnh (vế "ca đúng"). Vế **ca
biên** nằm ở dòng khác; ghi hết ở đây:

| lệnh | dòng | khẳng định biên là gì |
|---|---|---|
| `ls` | `v2/test/boot/shell-edge.sh:344` | `ls` trên đường không có ⇒ lỗi FS (đếm chung với `stat` trên cùng đường) |
| `cat` | `v2/test/boot/shell-edge.sh:175` | `cat` trần ⇒ đúng một dòng "thiếu đường dẫn", **không** dòng "lệnh lạ" nào |
| `cat` | `v2/test/boot/shell-edge.sh:441` | `cat` trên đường không có ⇒ đúng một dòng lỗi FS |
| `write` | `v2/test/boot/shell-edge.sh:174` | `write` trần ⇒ đúng một dòng "thiếu đường dẫn", **không** dòng "lệnh lạ" nào |
| `time` | `v2/test/boot/shell-edge.sh:283` | gọi hai lần ⇒ đúng hai dòng, và mốc sau **không nhỏ hơn** mốc trước |
| `uptime` | `v2/test/boot/shell-edge.sh:290` | gọi hai lần ⇒ đúng hai dòng, đơn điệu; `uptime` có tham số ⇒ bị coi là lệnh lạ |
| `sync` | `v2/test/boot/shell-edge.sh:295` | gọi hai lần ⇒ đúng hai dòng "sync ok" |
| `stat` | `v2/test/boot/shell-edge.sh:300` | `stat` trần ⇒ "thiếu đường dẫn"; `stat` trên đường không có ⇒ lỗi FS |
| `truncate` | `v2/test/boot/shell-edge.sh:247` | kích thước không phải số ⇒ báo lỗi; kích thước vượt trần ⇒ bị từ chối |
| `jobs` | `v2/test/boot/shell-edge.sh:257` | gọi hai lần ⇒ hai dòng **giống nhau từng ký tự**; `jobs` có tham số ⇒ bị coi là lệnh lạ |

## 3. Bằng chứng chạy thật trên ảnh sản phẩm

Ảnh `d2925f7e…` (build từ HEAD `ba8486ec`, pipeline canonical). Mười hai phép kiểm, tất cả xanh:

```text scope=kt138-soak-product/kt138-shell-edge.log
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 PASS
# shell-edge.sh: PASS
```

Bộ tự kiểm của chính harness (mỗi ca có một log sai phải đỏ và một log đúng phải xanh):

```text scope=kt138-soak-product/kt138-shell-edge-selftest.log
# shell-edge-selftest: PASS (8/8 ca, 31/31 kiểm: 18 sai→ĐỎ + 13 đúng→XANH)
```

## 4. Ba điều phải nói thẳng

**Một: `uptime` và `jobs` đúng là đã thiếu ca biên cho tới hôm nay.** Agent 0 chỉ đúng. Trước lượt
này, hai lệnh đó chỉ có ca đúng; ca biên `new-cmds-edge` **không** gửi chúng. Nay đã có ca
`new-cmds-edge2` và ca đó **đã chạy xanh trên ảnh sản phẩm** (không phải chỉ viết ra).

**Hai: bốn ca biên còn mỏng.** `write` mới có "thiếu đường dẫn" (chưa có đường rỗng, chưa có ca vượt
trần byte); `sync` mới có "gọi hai lần" (chưa có đường lỗi); `time` mới có "gọi hai lần" (chưa có ca
`time` kèm tham số); `ls` mới có "đường không có". Đây là **nợ đã biết**, không phải đã đủ.

**Ba: phán quyết của `new-cmds-ok` là gộp.** Khẳng định thì riêng cho từng lệnh (mỗi lệnh một phép
đếm), nhưng **một** dòng PASS cho **năm** lệnh, và log chỉ in lý do của lệnh hỏng **đầu tiên** — các
lệnh sau không được kiểm tiếp trong lượt đó. Muốn tách hẳn thì phải mỗi lệnh một lượt boot, tốn gấp
năm lần thời gian; hiện chưa làm.

## 5. Đính chính một con số tôi đã khai sai ở phiếu trước

Phiếu trước tôi viết "hai mươi bảy phép kiểm, trong đó mười chín ca âm". **Sai.** Số thật lúc đó là
**mười lăm âm + mười hai dương**. Con số mười chín là tôi chép nguyên từ dòng tổng kết **bị lỗi** của
chính script (nó lấy tổng trừ số ca, mà "số ca" đếm ca có số thứ tự chứ không đếm log-đúng). Đã sửa
script để in đúng hai vế; nay là **mười tám âm + mười ba dương** trên ba mươi mốt phép kiểm.

Bài học: chép số của công cụ mà không kiểm lại công cụ — đúng dạng "số viết tay" mà QD cấm.
