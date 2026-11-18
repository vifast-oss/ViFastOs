# KT-158 — seed thư mục `/docs` vào FS sản phẩm (nợ KT-137, D2-138)

**Ngày**: 2026-11-19 · **Task**: KT-158 (hàng chờ đêm vòng hai, D2-144..149) · **Nhánh**:
`night2/L2-fsdir` (tách từ `main` tại `6b2fdce7`) · **Worktree**: `/tmp/night2-L2-fsdir`.

**Bối cảnh đo được** (D2-138): `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs` hàm `seed()` **chỉ**
tạo `/README` ⇒ FS sản phẩm chỉ có gốc `/`. Vì vậy hai vế của ca `kt137-cdpwd` **không kiểm được**:
`cd` vào **thư mục con**, và đột biến "`pwd` in **hằng** `/`" **không phân biệt được** (mọi `pwd`
đều in `/` nên in hằng `/` cũng xanh).

**Commit**: xem `git log -1` trên nhánh — `[KT-158] seed /docs + /docs/note.txt vao FS san pham;
ca kt137-cdpwd kiem thu muc con (11->14 phep kiem); dot bien pwd in hang / ⇒ ca DO`.

---

## 1. Kết luận ngắn

- `seed()` nay tạo **ba** mục: `/README` (như cũ), `/docs` (`INODE_DIR`), `/docs/note.txt`
  (`INODE_FILE`, 16 B `ghi chu cua docs`), `fsync` sau mỗi lần tạo **như cũ**. Lỗi ở đây vẫn
  **không** làm mất FS (chỉ bỏ qua — giữ nguyên tinh thần hàm cũ).
- Ca `kt137-cdpwd` mở rộng **11 → 14 phép kiểm**: thêm `cd-docs`, `ls-docs`, `cat-docs`; `pwd` nay
  đòi **8** dòng `pwd /` + **2** dòng `pwd /docs` (tổng **10**); `cd-thu-tu` so **11** dòng.
- **Đột biến bắt buộc** (đúng điều D2-138 nêu): `cmd_pwd` in **hằng** `/` ⇒ ca **ĐỎ**
  (`kt137-cdpwd[pwd]: FAIL`). Trước KT-158 đột biến này **không phân biệt được**; nay phân biệt được.
- `bash v2/test/boot/shell-edge.sh --selftest` **PASS 42/42** (9/9 ca; 23 sai→ĐỎ + 19 đúng→XANH);
  ca 16 log sai nay **2 PASS + 12 FAIL**, ca 16 log đúng **14 PASS + 0 FAIL**.
- Ảnh sản phẩm mới (`--features product-shell,product-fs`): `shell-edge.sh --image` **PASS** toàn bộ
  (8/8 ca + `truncate-jobs` + `new-cmds-ok` + `new-cmds-edge` + `new-cmds-edge2` + `new-cmds-edge3` +
  `k7-uu-tien` + `k7-thu-muc` + `kt137-cdpwd`), ca 16 **14/14 xanh**.
- `./v2/bin/v2 check` **PASS** (HEAD `6b2fdce7` + thay đổi của task).
- **Không** sửa `spec/`: mục `ls` của `spec/v1-parity.toml` chỉ khớp `^# shell: ls n=[0-9]+ path=/`
  (**không** khớp nội dung) ⇒ thêm `/docs` **không** phá parity. Không sửa `docs/QUYET-DINH-*`,
  `docs/v2/ViFastOs-v2-thiet-ke.md`, `docs/v1/`.

## 2. Thay đổi

### 2.1 `v2/kernel/vifast-kbin-riscv64/src/proc_fs.rs`

- Thêm `const DOCS_NOTE: &[u8] = b"ghi chu cua docs";` (16 B, **không** `\n` cuối — `cat` in đúng
  một dòng nội dung rồi dòng tổng kết `# shell: cat ok bytes=16 first=ghi chu `).
- `seed()` thêm hai bước **sau** `/README`: `create("/docs", 0, INODE_DIR)` rồi
  `create("/docs/note.txt", 0, INODE_FILE)` + `write` + `fsync`. `create` trả `Exists` khi đĩa đã
  seed lần trước ⇒ bỏ qua, **không** ghi đè dữ liệu người dùng (cùng chính sách với `/README`).
- Import thêm `INODE_DIR` từ `vifast_fs`.
- **Không** đổi `TOTAL_BLOCKS`, **không** đổi `RAM_VOL_BLOCKS` (96), **không** đụng ngân sách
  `.data+.bss`: ba inode + ba block dữ liệu nằm trong volume sẵn có. Dòng `# product fs:` trên ảnh
  mới in `used=4` (trước `used=2`) — **không** harness nào khẳng định `used=` (đã rà: `t26.sh` chỉ
  đòi có `# product fs: medium=`; `shell-edge.sh --check-image` cũng chỉ đòi chuỗi đó).

### 2.2 `v2/test/boot/shell-edge.sh`

- `ok_kt137_pwd`: đếm **8** `# shell: pwd /` + **2** `# shell: pwd /docs` + **10** tổng. Vế `/docs`
  là vế **mới** phân biệt được đột biến `pwd` in hằng `/`.
- `ok_kt137_cd_thu_tu`: chuỗi kỳ vọng **7 → 11** dòng (thêm `cd /docs`, `cd khong co /docs/khongco`,
  `cd /` ×2).
- Ba hàm kiểm **mới**: `ok_kt137_cd_docs` (đúng 1 dòng `# shell: cd /docs`), `ok_kt137_ls_docs`
  (đúng 1 dòng `# shell: ls n=1 path=/docs` + bảng có `  f note.txt`), `ok_kt137_cat_docs` (đúng 1
  dòng nội dung `ghi chu cua docs` + đúng 1 dòng `# shell: cat ok bytes=16 first=ghi chu…`).
- `check_kt137_cdpwd`: **11 → 14** cặp phán quyết.
- `run_real` ca 16: thêm **10** lệnh cuối ca — `cd /docs` `pwd` `ls` `cat /docs/note.txt`
  `cd /docs/khongco` `pwd` `cd ..` `pwd` `cd /docs/..` `pwd`.
- `--selftest` ca 16: log giả **sai** thêm khối "trước KT-158 (FS **không** có `/docs`)": `cd /docs`
  báo không có, `ls`/`cat` trong `/docs` đều `fs loi code=5`, `pwd` **không** in `/docs` lần nào ⇒
  **12** phép kiểm đỏ, 2 xanh (`cd-khong-co`, `ls-cwd` — vẫn có ca âm riêng). Log giả **đúng** cập
  nhật theo đầu ra **thật** của ảnh mới (`ino=7`, `ls n=5 path=/`, `  d docs`, khối `/docs`).
  Đếm kỳ vọng: `(sai) 1 2 12`, `(dung) 0 14 0`. Tổng số phép kiểm selftest **không đổi 42** (không
  thêm cặp `selftest_pair` nào — ba phép kiểm mới đã **đỏ** trong log sai nên đã được chứng minh).

## 3. Bằng chứng — ảnh sản phẩm **mới** (xanh)

Lệnh:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
VIFAST_REPRO_CARGO_EXTRA="--features product-shell,product-fs" bash v2/tools/repro-build.sh
bash v2/test/boot/shell-edge.sh --image <ảnh>
```

**Lượt đo chính** (build **sau khi commit**, cây sạch ⇒ `dirty=0`, danh tính khớp đúng HEAD
`399d5ac0`):

```
# image: sha256=735e1030bbeec74f46a0a94297266e8103d39e4083e1c8d2935e323f729ad27e bytes=209536
# image: # build: algo=kt134-v4 inputs=0dd9e10ec24a0024 dirty=0
```

(log `kt158-logs/kt158-build-clean.log` + `kt158-logs/kt158-shell-edge-clean.log`)

Lượt đo **trước** commit (cùng nội dung nguồn, `dirty=1`) cũng xanh — log
`kt158-logs/kt158-build-green.log` / `kt158-logs/kt158-shell-edge-green.log`, ảnh
`sha256=d7b7284bf22b8fc9211afd8b49464df6156164568793269a1120f514b295a247 size=209536
build=f1d0667d8cdb07bd text=130934 inputs=0b8016e6cb6038d5 dirty=1 rev=6b2fdce7ae28`.

Dòng `# product fs:` (đổi `used=2` → `used=4` vì thêm ba mục):

```
# product fs: medium=ram blocks=96 used=4 total=66 volatile=1 why=khong-co-pci const:ram_pa=0x83000000 const:ram_blocks=96 const:reply_bytes=24 const:path_max=48
```

`ls /` trên ảnh mới (thấy `docs`):

```
  f README
  d docs
  f a
  f g
  f rel.txt
# shell: ls n=5 path=/
```

Khối `/docs` của ca 16 (dán nguyên văn từ `kt158-logs/run-clean/qemu.log`):

```
# shell: cd /docs
shell>
# shell: pwd /docs
shell>
  f note.txt
# shell: ls n=1 path=/docs
shell>
ghi chu cua docs
# shell: cat ok bytes=16 first=ghi chu 
shell>
# shell: cd khong co /docs/khongco
shell>
# shell: pwd /docs
shell>
# shell: cd /
shell>
# shell: pwd /
shell>
# shell: cd /
shell>
# shell: pwd /
```

Mười bốn dòng phán quyết của ca 16 (log `kt158-logs/kt158-shell-edge-clean.log`):

```
# kt137-cdpwd[pwd]: PASS
# kt137-cdpwd[cd-khong-co]: PASS
# kt137-cdpwd[cd-tep]: PASS
# kt137-cdpwd[cd-thu-tu]: PASS
# kt137-cdpwd[cat-tuong-doi]: PASS
# kt137-cdpwd[stat-tuong-doi]: PASS
# kt137-cdpwd[write-tuong-doi]: PASS
# kt137-cdpwd[cat-rel]: PASS
# kt137-cdpwd[truncate-tuong-doi]: PASS
# kt137-cdpwd[stat-rel]: PASS
# kt137-cdpwd[ls-cwd]: PASS
# kt137-cdpwd[cd-docs]: PASS
# kt137-cdpwd[ls-docs]: PASS
# kt137-cdpwd[cat-docs]: PASS
# kt137-cdpwd: PASS (# shell: pwd /)
```

Toàn bộ lượt chạy:

```
# shell-edge: 8/8 ca + truncate-jobs + new-cmds-ok + new-cmds-edge + new-cmds-edge2 + new-cmds-edge3 + k7-uu-tien + k7-thu-muc + kt137-cdpwd PASS
# shell-edge.sh: PASS
```

## 4. Bằng chứng — **đột biến** `pwd` in hằng `/` (đỏ)

Đột biến (tạm thời, **đã revert**, không commit):

```rust
fn cmd_pwd(_cwd: &Cwd) {
    put(b"# shell: pwd /\n");
}
```

Ảnh đột biến: `sha256=2f7fe7a289d8cd301e1258921c486ba5f152b6c9c1fabb25c02176b0c873aa1b size=209536`
(log `kt158-logs/kt158-build-mut.log`). Trên ảnh đó, `cd /docs` **thành công** nhưng `pwd` vẫn in
`/` — đúng hành vi D2-138 nói là **không phân biệt được** trước đây:

```
# shell: cd /docs
shell>
# shell: pwd /
shell>
  f note.txt
```

Đếm trên log đột biến: `grep -c -x -F '# shell: pwd /'` = **10**, `grep -c -x -F '# shell: pwd /docs'`
= **0**.

Ca 16 **ĐỎ** (dán nguyên văn từ `kt158-logs/kt158-shell-edge-mut.log`):

```
# kt137-cdpwd[pwd]: FAIL (mong đúng 8 dòng '# shell: pwd /', thấy 10)
# kt137-cdpwd[cd-khong-co]: PASS
# kt137-cdpwd[cd-tep]: PASS
# kt137-cdpwd[cd-thu-tu]: PASS
# kt137-cdpwd[cat-tuong-doi]: PASS
# kt137-cdpwd[stat-tuong-doi]: PASS
# kt137-cdpwd[write-tuong-doi]: PASS
# kt137-cdpwd[cat-rel]: PASS
# kt137-cdpwd[truncate-tuong-doi]: PASS
# kt137-cdpwd[stat-rel]: PASS
# kt137-cdpwd[ls-cwd]: PASS
# kt137-cdpwd[cd-docs]: PASS
# kt137-cdpwd[ls-docs]: PASS
# kt137-cdpwd[cat-docs]: PASS
# kt137-cdpwd: FAIL (xem các dòng [<lệnh>]: FAIL ở trên)
```

```
# shell-edge: có ca ĐỎ (xem log /tmp/night2-L2-fsdir/v2/agent/JOURNAL/kt158-logs/run-mut/qemu.log)
# shell-edge.sh: FAIL
```

**Chỉ** phép kiểm `pwd` đỏ, 13 phép kiểm còn lại xanh ⇒ phán quyết **quy đúng lệnh**, không phải
"đỏ cả ca". Đột biến đã được **revert** trước khi commit (`git diff` chỉ còn `proc_fs.rs` +
`shell-edge.sh`).

## 5. `--selftest` (không cần ảnh)

```
# selftest 16 kt137-cdpwd (sai): ĐÚNG (2 PASS + 12 FAIL, mã thoát 1)
# selftest 16 kt137-cdpwd (dung): ĐÚNG (14 PASS + 0 FAIL, mã thoát 0)
# selftest 16 cd-khong-co: log SAI -> ĐỎ đúng (mong đúng 1 dòng '# shell: cd khong co /khongco', thấy 2)
# selftest 16 cd-khong-co: log ĐÚNG -> XANH đúng
# selftest 16 ls-cwd: log SAI -> ĐỎ đúng (mong đúng 2 dòng '# shell: ls n=<n> path=/' (ls + ls .), thấy 1)
# selftest 16 ls-cwd: log ĐÚNG -> XANH đúng
# shell-edge-selftest: PASS (9/9 ca, 42/42 kiểm: 23 sai→ĐỎ + 19 đúng→XANH)
```

## 6. `./v2/bin/v2 check`

```
== v2 check (HEAD 6b2fdce7) ==
--- fmt: OK
...
--- selftest shell-edge (KT-108): OK
...
--- build (L0 compile): OK
v2 check: PASS (L1 chạy trong gate --fast)
```

Log đầy đủ: `v2/agent/JOURNAL/kt158-logs/kt158-v2-check.log`.

## 7. `persist.sh` — đã kiểm, **không** phải sửa

`proc_fs::seed` chỉ được biên dịch vào **ảnh sản phẩm** (`src/main.rs:435 mod proc_fs;`);
`ktest_persist.rs` **không** tham chiếu `proc_fs` (`grep -c proc_fs ktest_persist.rs` = 0). Runner
`persist.sh` kiểm ảnh đĩa bằng `vifast-fsutil ls/stat/get` trên **ảnh ktest-persist**, không đọc
`ls /` của shell sản phẩm; `persist_host.py` không có `README`/`docs`. Vì vậy thêm `/docs` **không**
ảnh hưởng `persist.sh` — không nới ngưỡng nào.

## 8. Còn thiếu / ghi chú

- **Không** còn nợ kỹ thuật nào của phạm vi task. `spec/` **không** phải sửa (mục `ls` của
  `spec/v1-parity.toml` không khớp nội dung).
- `v2/docs/k6-product-fs.md` có **log mẫu cũ** in `used=2` (dòng 103/107/111/140/148/223). Đây là
  tài liệu **đo** của KT-77c, **không** thuộc phạm vi task và **không** có cổng nào đối chiếu
  (`grep -rn k6-product-fs` trong `v2/tools/`/`v2/test/` = 0). Không sửa để tránh đụng tài liệu
  ngoài phạm vi; nếu Agent 0 muốn cập nhật thì đó là việc riêng (kèm log đo mới).
- Ảnh dùng để đo **chính** (sau commit) mang `dirty=0`, `inputs=0dd9e10ec24a0024`, `rev=399d5ac08fe8`
  — danh tính khớp đúng HEAD đã commit. Lượt đo **trước** commit (`dirty=1`) cũng xanh và được giữ
  làm log phụ; `shell-edge.sh` chỉ đòi `algo=` khớp định nghĩa hiện hành, không đòi `dirty=0`
  (`vbrev_gate` mới đòi — `shell-edge` không dùng `vbrev_gate`).
- Một QEMU mỗi lượt, `pgrep qemu-system | wc -l` = 0 trước và sau mỗi lượt; không `pkill -f
  qemu-system`.
