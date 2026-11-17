<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->
# ĐÓNG BĂNG `main` cho dòng ViScript — 17/11/2026
**Yêu cầu**: dòng OS (KT-88) cần HEAD `main` đứng yên để cổng `# build: rev` chấp nhận ảnh vừa build (đo cuối K6).
**Hiệu lực**: từ **15:27 JST** đến **16:27 JST** (60 phút), có thể kết thúc sớm nếu dòng OS báo xong.
**Trong thời gian đóng băng**:
- **Không** commit, **không** push vào `main` từ dòng ViScript.
- **Tạm dừng** các lệnh nặng trên mac (gate `--fast`, soak, `cargo build` lớn) để không nhiễu phép đo K6; việc nhẹ/đọc tệp vẫn làm.
- ST-175 đang dở: **giữ nguyên working tree**, không commit; sẽ commit sau khi hết đóng băng.
**Sau đóng băng**: commit ST-175 → chạy **ST-176** (#147) → **ST-170 (R29)** → **ST-171** (tách repo) → **ST-178** → **ST-179+**.
