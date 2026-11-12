// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
use super::std;
use super::*;
/// P1-2 (r290, đỏ-trước): chỉ số block bịa trong inode phải trả `Corrupt`,
/// không được panic. Bản cũ tính chỉ số từ `inode.size` (dữ liệu trên đĩa)
/// rồi đọc thẳng bảng indirect ⇒ `index out of bounds`.
#[test]
fn read_with_a_forged_size_returns_corrupt_instead_of_panicking() {
    let mut fs = fresh();
    let ino = fs.create(b"/forged", 0, INODE_FILE).unwrap();
    // Đủ block để inode có bảng indirect thật (vượt vùng direct).
    let direct = fs.direct_blocks;
    let payload = vec![0xa5u8; BLOCK_SIZE * (direct + 2)];
    fs.write(ino, 0, &payload, 0).unwrap();
    assert_ne!(
        fs.inodes[ino as usize - 1].blocks[direct],
        0,
        "phải có indirect"
    );
    // Bịa: size khổng lồ + con trỏ double-indirect rác (đĩa hỏng/độc).
    fs.inodes[ino as usize - 1].size = u32::MAX;
    fs.inodes[ino as usize - 1].blocks[direct + 1] = 7;
    let mut out = [0u8; 32];
    let err = fs.read(ino, 0x3fff_f000, &mut out, 0);
    assert!(
        matches!(err, Err(FsError::Corrupt) | Err(FsError::Invalid)),
        "phải trả lỗi, không panic; nhận {:?}",
        err
    );
}

/// r296b (T26, test hồi quy — đã từng ĐỎ trước khi vá): readdir theo
/// **chỉ số entry** bỏ qua một entry khi một entry phía trước bị xoá GIỮA
/// hai lời gọi.
///
/// Đây đúng là thứ đo được trên QEMU: client đi `/` bằng `readdir(fd, 0),
/// (fd, 1), …` trong khi root tạo rồi xoá `/rm-test` trong CÙNG thư mục.
/// Khi `/rm-test` (ngay trước `d`) biến mất, `d` tụt từ chỉ số 6 xuống 5 —
/// mà 5 đã đọc rồi ⇒ **mất `d`** dù dữ liệu còn nguyên.
///
/// Test này KHÔNG nằm trong cổng mặc định vì nó đỏ trên mã hiện tại; chạy
/// đích danh để chứng minh:
/// Trước khi vá nó đỏ (chạy đích danh): thấy [e0,e1,e2,e3,e4,rm-test] và
/// **mất `d`**. Sau khi vá (chỉ số là SLOT) nó xanh — đây là test hồi quy.
#[test]
fn readdir_must_not_skip_a_later_entry_when_an_earlier_one_is_removed() {
    let mut fs = fresh();
    // Thư mục GỐC (ino 1) — đúng thư mục mà client liệt kê.
    for name in [
        &b"/e0"[..],
        &b"/e1"[..],
        &b"/e2"[..],
        &b"/e3"[..],
        &b"/e4"[..],
        &b"/rm-test"[..],
        &b"/d"[..],
    ] {
        fs.create(name, 0, INODE_FILE).unwrap();
    }
    let mut seen: Vec<Vec<u8>> = Vec::new();
    let mut index = 0usize;
    loop {
        match fs.read_dir_entry(1, index) {
            Ok(Some((_, name, len))) => {
                seen.push(name[..len].to_vec());
                // root xoá "/rm-test" NGAY SAU khi client vừa đọc nó và
                // TRƯỚC khi client đọc chỉ số kế tiếp.
                if name[..len] == *b"rm-test" {
                    fs.unlink(b"/rm-test", 0).unwrap();
                }
                index += 1;
            }
            Ok(None) => break,
            Err(error) => panic!("readdir lỗi bất ngờ: {error:?}"),
        }
    }
    assert!(
            seen.iter().any(|name| name == b"d"),
            "liệt kê theo chỉ số ĐÃ BỎ SÓT entry `d` sau khi một entry phía trước bị xoá; thực tế: {seen:?}"
        );
}

/// 291.3 (r291, đỏ-trước): ghi quá EOF tạo LỖ; đọc phần lỗ phải ra 0 và
/// cả file vẫn đọc được. Bản cũ: `read` thấy block 0 ⇒ `Corrupt` ⇒ ghi xa
/// EOF làm hỏng toàn bộ file.
#[test]
fn writing_past_eof_leaves_a_readable_zero_filled_hole() {
    let mut fs = fresh();
    let ino = fs.create(b"/sparse", 0, INODE_FILE).unwrap();
    let payload = [0x7au8; 16];
    // 64 KiB = block 128 ⇒ nằm trong vùng single-indirect.
    let offset = 64 * 1024;
    assert_eq!(fs.write(ino, offset, &payload, 0).unwrap(), payload.len());

    let mut out = [0u8; 16];
    assert_eq!(fs.read(ino, offset, &mut out, 0).unwrap(), 16);
    assert_eq!(out, payload, "phần đã ghi phải đúng");

    let mut hole = [0xffu8; 64];
    assert_eq!(fs.read(ino, 0, &mut hole, 0).unwrap(), 64);
    assert!(hole.iter().all(|b| *b == 0), "phần lỗ phải đọc ra 0");
    fs.fsck().unwrap();
}

/// 291.4 (r291, đỏ-trước): `create` hỏng giữa đường (thư mục đầy) phải
/// ROLLBACK inode đã cấp — nếu không, lần commit sau ghi một inode mồ côi
/// (không entry nào trỏ tới) và bộ đếm inode của FS bị rò rỉ.
#[test]
fn failed_create_rolls_back_the_allocated_inode() {
    // Thư mục gốc chỉ 1 block (12 entry) nhưng 64 inode ⇒ thư mục ĐẦY
    // trước khi cạn inode, đúng nhánh `write_dir_entry` hỏng.
    let mut dev = MemDev::new();
    format_with_options(
        &mut dev,
        FormatOptions {
            total_blocks: 0,
            inode_limit: MAX_INODES as u16,
            dir_block_limit: 1,
        },
    )
    .unwrap();
    let mut fs = Fs::mount(dev).unwrap();
    let alive = |fs: &Fs<MemDev>| {
        fs.inodes
            .iter()
            .filter(|inode| inode.kind != INODE_FREE)
            .count()
    };
    let baseline = alive(&fs);
    let mut created = 0usize;
    let mut failure = None;
    let mut index = 0usize;
    while index < 400 {
        // Tên DÀI để thư mục đầy trước khi cạn inode (mỗi entry ~100 B).
        let name = std::format!("/{}{index:02}", "n".repeat(24));
        match fs.create(name.as_bytes(), 0, INODE_FILE) {
            Ok(_) => created += 1,
            Err(error) => {
                failure = Some(error);
                break;
            }
        }
        index += 1;
    }
    let error = failure.expect("phải chạm giới hạn thư mục");
    assert!(
        matches!(error, FsError::NoSpace),
        "lỗi phải là NoSpace: {error:?}"
    );
    assert!(created > 0);
    assert_eq!(
        alive(&fs),
        baseline + created,
        "create hỏng không được để lại inode mồ côi (rò rỉ)"
    );
    // Và FS vẫn lành: một create mới thành công rồi fsck sạch.
    fs.fsck().unwrap();
}

/// P1-1 (r290, đỏ-trước): đĩa LỚN HƠN vùng đã format phải mount được.
///
/// Kịch bản thật (bản soát 2026-11-10): `dd` 4 MiB → `mkfs` → `truncate -s
/// 8M` → mount. Bản cũ lấy `dev.block_count()` làm biên nên quét bitmap
/// quá vùng FS và panic `index out of bounds` (8 MiB = 16 384 block, bitmap
/// chỉ phủ 8 192 block).
#[test]
fn mount_accepts_a_device_larger_than_the_formatted_area() {
    let mut small = MemDev::with_blocks(4_096);
    format(&mut small).unwrap();
    // "truncate -s 8M": cùng nội dung, thiết bị dài gấp bốn.
    let mut data = small.data;
    data.resize(16_384 * BLOCK_SIZE, 0);
    let big = MemDev { data };

    let mut fs = Fs::mount(big).expect("mount phải chấp nhận đĩa lớn hơn");
    // Biên cấp phát là vùng đã format (4 096), không phải 16 384.
    let (_, total) = fs.usage();
    assert_eq!(total, 4_096 - DATA_START);
    // Và FS vẫn dùng được bình thường.
    let ino = fs.create(b"/big", 0, INODE_FILE).unwrap();
    fs.write(ino, 0, &[0x5au8; BLOCK_SIZE], 0).unwrap();
    fs.fsck().unwrap();
}

#[test]
fn fsck_detects_corrupt_bitmap_and_dangling_dir_entry() {
    // Vòng 217: `fsck()` được gọi ~42 lần trong test nhưng CHƯA BAO GIỜ được
    // chứng minh là BẮT ĐƯỢC hỏng — giống hệt `check_invariants` ở vòng 189.
    // Lưu ý: fsck kiểm theo `self.bitmap` trong bộ nhớ, nên mutation phải
    // sửa bitmap TRONG BỘ NHỚ, không phải block trên dev.
    let mut fs = fresh();
    let ino = fs.create(b"/keep", 0, INODE_FILE).unwrap();
    fs.write(ino, 0, &[0x11u8; BLOCK_SIZE], 0).unwrap();
    fs.fsck().unwrap();

    // (a) Bitmap nói block của file là FREE trong khi inode vẫn trỏ tới nó.
    let block = fs.inodes[ino as usize - 1].blocks[0];
    assert_ne!(block, 0);
    let byte = block as usize / 8;
    let bit = 1u8 << (block % 8);
    fs.bitmap[byte] &= !bit;
    assert_eq!(
        fs.fsck(),
        Err(FsError::Corrupt),
        "bitmap thiếu block phải bị bắt"
    );

    // Khôi phục rồi phá kiểu khác: entry trỏ tới inode đã free.
    fs.bitmap[byte] |= bit;
    fs.fsck().unwrap();
    fs.inodes[ino as usize - 1].kind = INODE_FREE;
    assert_eq!(
        fs.fsck(),
        Err(FsError::Corrupt),
        "dangling dir entry phải bị bắt"
    );
}

#[test]
fn no_space_is_reported_and_old_data_survives() {
    // Vòng 216: đĩa ĐẦY phải báo `NoSpace` gọn, và dữ liệu đã ghi trước đó
    // phải còn đọc được. Đây là loại lỗi mà "hết chỗ" thường làm hỏng file
    // dở dang thay vì từ chối sạch.
    let mut dev = MemDev::with_blocks(64);
    format(&mut dev).unwrap();
    let mut fs = Fs::mount(dev).unwrap();
    let keep = fs.create(b"/keep", 0, INODE_FILE).unwrap();
    fs.write(keep, 0, b"before-full", 0).unwrap();
    // Ghi một file lớn cho tới khi hết chỗ.
    let big = fs.create(b"/big", 0, INODE_FILE).unwrap();
    let chunk = [0x5Au8; BLOCK_SIZE];
    let mut written = 0usize;
    let mut full_error = None;
    while written < 512 * BLOCK_SIZE {
        match fs.write(big, written, &chunk, 0) {
            Ok(n) => written += n,
            Err(error) => {
                full_error = Some(error);
                break;
            }
        }
    }
    assert_eq!(
        full_error,
        Some(FsError::NoSpace),
        "ghi tới khi hết đĩa phải trả NoSpace, không phải lỗi khác"
    );
    // Dữ liệu cũ phải nguyên vẹn và FS vẫn nhất quán.
    let mut buf = [0u8; 16];
    let n = fs.read(keep, 0, &mut buf, 0).unwrap();
    assert_eq!(&buf[..n], b"before-full");
    fs.fsck().unwrap();
}

#[test]
fn other_uid_is_read_only_and_cannot_unlink() {
    // Vòng 215: kiểm ĐƯỜNG THẤT BẠI của quyền hạn (phần lớn test trước đây
    // chỉ đi đường thành công). Mode mặc định `0b0111` = owner rwx, other
    // chỉ đọc ⇒ uid khác ghi phải bị từ chối nhưng đọc vẫn được.
    let mut fs = fresh();
    // Tạo bằng uid 0: thư mục gốc thuộc uid 0, mode `0b0111` ⇒ uid khác
    // KHÔNG được tạo entry trong đó (chính điều này làm bản nháp của test
    // thất bại ở `create` — đó là hành vi ĐÚNG của FS, không phải lỗi).
    let ino = fs.create(b"/p", 0, INODE_FILE).unwrap();
    fs.write(ino, 0, b"secret", 0).unwrap();
    assert_eq!(fs.write(ino, 0, b"x", 7), Err(FsError::NoAccess));
    let mut buf = [0u8; 8];
    let n = fs.read(ino, 0, &mut buf, 7).unwrap();
    assert_eq!(&buf[..n], b"secret", "other vẫn phải ĐỌC được");
    // Xoá entry đòi quyền ghi trên thư mục cha (thuộc uid 0).
    assert_eq!(fs.unlink(b"/p", 7), Err(FsError::NoAccess));
    assert!(
        fs.lookup(b"/p", 5).is_ok(),
        "file còn nguyên sau khi bị từ chối"
    );
    // uid 0 (root) làm được mọi thứ — dùng để dọn.
    fs.unlink(b"/p", 0).unwrap();
}

#[test]
fn truncate_shrinks_and_grows_with_zero_fill() {
    // Vòng 214: `truncate` chưa có test — nếu không trả block khi cắt thì
    // `df` giảm dần; nếu không zero phần đuôi thì đọc ra dữ liệu cũ (rò rỉ
    // nội dung giữa hai file trên cùng block).
    let mut fs = fresh();
    let ino = fs.create(b"/t", 0, INODE_FILE).unwrap();
    fs.write(ino, 0, &[0xAAu8; 3 * BLOCK_SIZE], 0).unwrap();
    let (used_full, _) = fs.usage();
    // Cắt còn 1 block: block phải được trả và size đúng.
    fs.truncate(ino, BLOCK_SIZE, 0).unwrap();
    assert_eq!(fs.info(ino).unwrap().size as usize, BLOCK_SIZE);
    let (used_one, _) = fs.usage();
    assert!(used_one < used_full, "cắt bớt block phải làm used giảm");
    let mut buf = [0u8; BLOCK_SIZE * 2];
    let n = fs.read(ino, 0, &mut buf, 0).unwrap();
    assert_eq!(n, BLOCK_SIZE, "đọc sau khi cắt chỉ trả phần còn lại");
    assert!(buf[..n].iter().all(|&b| b == 0xAA));
    // Mở rộng lại: phần mới phải là 0, không phải rác cũ.
    fs.truncate(ino, 2 * BLOCK_SIZE, 0).unwrap();
    assert_eq!(fs.info(ino).unwrap().size as usize, 2 * BLOCK_SIZE);
    let mut grown = [0xFFu8; BLOCK_SIZE * 2];
    let n = fs.read(ino, 0, &mut grown, 0).unwrap();
    assert_eq!(n, 2 * BLOCK_SIZE);
    assert!(grown[..BLOCK_SIZE].iter().all(|&b| b == 0xAA));
    assert!(
        grown[BLOCK_SIZE..].iter().all(|&b| b == 0),
        "phần mở rộng phải bằng 0, không phải dữ liệu cũ"
    );
    // Cắt về 0 phải trả hết block của file.
    fs.truncate(ino, 0, 0).unwrap();
    assert_eq!(fs.info(ino).unwrap().size, 0);
    let (used_zero, _) = fs.usage();
    assert!(used_zero < used_one);
}

#[test]
fn usage_tracks_allocation_and_unlink() {
    // Vòng 213: `df` (STATFS) lấy số liệu từ `usage()`; nếu bitmap không
    // nhả block khi xoá thì `df` báo thiếu chỗ dần dần — rất khó thấy.
    let mut fs = fresh();
    // Mốc so sánh lấy SAU khi đã có một file: block của thư mục gốc (nếu
    // được cấp) không phải thứ ta muốn đo — thứ cần đo là block của FILE.
    let keep = fs.create(b"/keep", 0, INODE_FILE).unwrap();
    fs.write(keep, 0, b"x", 0).unwrap();
    let (used0, total0) = fs.usage();
    let ino = fs.create(b"/f", 0, INODE_FILE).unwrap();
    fs.write(ino, 0, &[7u8; 3 * BLOCK_SIZE], 0).unwrap();
    let (used1, total1) = fs.usage();
    assert!(used1 > used0, "ghi 3 block phải làm used tăng");
    assert_eq!(total1, total0);
    fs.unlink(b"/f", 0).unwrap();
    let (used2, total2) = fs.usage();
    assert_eq!(
        used2, used0,
        "xoá file phải trả block về bitmap (nếu không, df báo thiếu chỗ dần)"
    );
    assert_eq!(total2, total0, "tổng dung lượng không đổi");
}

#[test]
fn rename_moves_entry_and_rejects_missing_or_duplicate() {
    // Vòng 212: `rename` chưa có test nào — trong khi nó là lệnh dễ làm hỏng
    // FS nhất (đổi tên xong mà journal sai là mất cả cây).
    let mut fs = fresh();
    fs.create(b"/a.txt", 0, INODE_FILE).unwrap();
    fs.rename(b"/a.txt", b"/b.txt", 0).unwrap();
    assert!(fs.lookup(b"/a.txt", 0).is_err(), "tên cũ phải biến mất");
    assert!(fs.lookup(b"/b.txt", 0).is_ok(), "tên mới phải tra được");
    // Đích không tồn tại nguồn ⇒ NotFound; đích đã tồn tại ⇒ Exists.
    assert_eq!(fs.rename(b"/missing", b"/x", 0), Err(FsError::NotFound));
    fs.create(b"/c.txt", 0, INODE_FILE).unwrap();
    assert_eq!(fs.rename(b"/b.txt", b"/c.txt", 0), Err(FsError::Exists));
    // Dữ liệu phải còn nguyên sau khi đổi tên (ghi trước, đọc sau).
    let ino = fs.lookup(b"/c.txt", 0).unwrap();
    fs.write(ino, 0, b"hello", 0).unwrap();
    fs.rename(b"/c.txt", b"/d.txt", 0).unwrap();
    let ino = fs.lookup(b"/d.txt", 0).unwrap();
    let mut buf = [0u8; 8];
    let n = fs.read(ino, 0, &mut buf, 0).unwrap();
    assert_eq!(&buf[..n], b"hello");
}

#[test]
fn rename_rejects_directory_cycles_and_dotdot() {
    // Vòng 212: đổi tên thư mục vào CHÍNH cây con của nó sẽ tạo chu trình —
    // mount sau đó không dạo được cây. `..` bị `next_component` từ chối nên
    // không thể lách bằng đường dẫn tương đối.
    let mut fs = fresh();
    fs.mkdir(b"/a", 0).unwrap();
    fs.mkdir(b"/a/b", 0).unwrap();
    assert_eq!(fs.rename(b"/a", b"/a/b", 0), Err(FsError::Invalid));
    assert_eq!(fs.rename(b"/a", b"/a/b/new", 0), Err(FsError::Invalid));
    assert_eq!(fs.rename(b"/a", b"/x/../y", 0), Err(FsError::Invalid));
    // Đổi tên thư mục bình thường vẫn phải chạy.
    fs.rename(b"/a", b"/z", 0).unwrap();
    assert!(fs.lookup(b"/z/b", 0).is_ok(), "con phải còn nguyên");
}

#[test]
fn cached_device_hits_and_write_through() {
    let mut cache = CachedDev::new(CountingDev::new());
    let data = [0xab; BLOCK_SIZE];
    cache.write(3, &data).unwrap();
    assert_eq!(cache.inner().writes, 1);

    let mut out = [0u8; BLOCK_SIZE];
    cache.read(3, &mut out).unwrap();
    assert_eq!(out, data);
    assert_eq!(cache.stats().hits, 1);
    assert_eq!(cache.stats().misses, 0);
    assert_eq!(cache.inner().reads, 0);

    cache.read(4, &mut out).unwrap();
    assert_eq!(out, [0u8; BLOCK_SIZE]);
    assert_eq!(cache.stats().hits, 1);
    assert_eq!(cache.stats().misses, 1);
    assert_eq!(cache.inner().reads, 1);
}

#[test]
fn cached_device_write_backs_file_data_only() {
    let mut cache = CachedDev::new(CountingDev::new());
    let data = [0x5a; BLOCK_SIZE];
    cache.write(DATA_START, &data).unwrap();
    assert_eq!(cache.inner().writes, 0);
    let mut out = [0u8; BLOCK_SIZE];
    cache.read(DATA_START, &mut out).unwrap();
    assert_eq!(out, data);
    cache.flush().unwrap();
    assert_eq!(cache.inner().writes, 1);
    assert_eq!(cache.inner().flushes, 1);

    // Journal/metadata blocks stay write-through even with caching.
    cache.write(JOURNAL_START, &data).unwrap();
    assert_eq!(cache.inner().writes, 2);
}

#[test]
fn cached_device_evicts_round_robin() {
    let mut cache = CachedDev::new(CountingDev::new());
    let mut out = [0u8; BLOCK_SIZE];
    for lba in 0..CACHE_SLOTS as u32 {
        cache.write(lba + 1, &[lba as u8; BLOCK_SIZE]).unwrap();
    }
    cache.read(1, &mut out).unwrap();
    assert_eq!(out[0], 0);
    assert_eq!(cache.stats().hits, 1);

    // The next miss replaces slot 0 (LBA 1).
    cache.read(CACHE_SLOTS as u32 + 1, &mut out).unwrap();
    cache.read(1, &mut out).unwrap();
    assert_eq!(cache.stats().misses, 2);
}

#[test]
fn cached_device_wraps_filesystem() {
    let mut dev = CountingDev::new();
    format(&mut dev).unwrap();
    let mut fs = Fs::mount(CachedDev::new(dev)).unwrap();
    let file = fs.create(b"/cache", 0, INODE_FILE).unwrap();
    fs.write(file, 0, b"through-cache", 0).unwrap();

    let mut out = [0u8; 13];
    fs.read(file, 0, &mut out, 0).unwrap();
    assert_eq!(&out, b"through-cache");
    assert!(fs.dev().stats().hits > 0);
    assert!(fs.dev().stats().writes > 0);
    fs.fsync().unwrap();
    assert!(fs.dev().inner().flushes >= 1);
    fs.fsck().unwrap();
}

#[test]
fn basic_namespace_and_data() {
    let mut fs = fresh();
    fs.fsck().unwrap();
    let dir = fs.mkdir(b"/d", 1).unwrap();
    let file = fs.create(b"/d/a", 1, INODE_FILE).unwrap();
    let data = [0x5au8; 600];
    fs.write(file, 0, &data, 1).unwrap();
    let mut out = [0u8; 600];
    assert_eq!(fs.read(file, 0, &mut out, 1).unwrap(), 600);
    assert_eq!(out, data);
    assert_eq!(fs.info(file).unwrap().size, 600);
    assert_eq!(fs.read_dir_entry(ROOT_INO, 0).unwrap().unwrap().0, dir);
    assert_eq!(fs.read_dir_entry(dir, 0).unwrap().unwrap().0, file);
    fs.rename(b"/d/a", b"/d/b", 1).unwrap();
    assert!(fs.lookup(b"/d/a", 1).is_err());
    assert_eq!(fs.lookup(b"/d/b", 1).unwrap(), file);
    fs.truncate(file, 100, 1).unwrap();
    assert_eq!(fs.info(file).unwrap().size, 100);
    fs.fsck().unwrap();
    let mut fs2 = reopen(&fs);
    fs2.fsck().unwrap();
    assert_eq!(fs2.lookup(b"/d/b", 1).unwrap(), file);
    fs2.unlink(b"/d/b", 1).unwrap();
    fs2.unlink(b"/d", 1).unwrap();
    fs2.fsck().unwrap();
}

#[test]
fn journal_replay_and_torn_commit() {
    let mut fs = fresh();
    let file = fs.create(b"/j", 0, INODE_FILE).unwrap();
    fs.write(file, 0, b"old", 0).unwrap();
    let bytes = *b"new-data";
    // Build a pending write and leave the commit for mount to replay.
    fs.inject_pending_write(b"/j", &bytes, 0, false).unwrap();
    let mut fs2 = reopen(&fs);
    assert_eq!(fs2.replayed(), 1);
    let mut out = [0u8; 8];
    assert_eq!(fs2.read(file, 0, &mut out, 0).unwrap(), 8);
    assert_eq!(&out, &bytes);
    fs2.fsck().unwrap();

    // Torn commit must be ignored and the old target data preserved.
    fs.write(file, 0, b"stable", 0).unwrap();
    fs.inject_pending_write(b"/j", b"must-not-land", 0, true)
        .unwrap();
    let mut fs3 = reopen(&fs);
    assert_eq!(fs3.replayed(), 2);
    let mut out = [0u8; 6];
    assert_eq!(fs3.read(file, 0, &mut out, 0).unwrap(), 6);
    assert_eq!(&out, b"stable");
    fs3.fsck().unwrap();
}

#[test]
fn vf03_image_still_mounts_with_legacy_geometry() {
    let mut dev = MemDev::new();
    format(&mut dev).unwrap();
    let mut superblock = zero_block();
    dev.read(1, &mut superblock).unwrap();
    superblock[..4].copy_from_slice(b"VF03");
    put_u32(&mut superblock, 4, 3);
    let crc = crc32(&superblock[..56]);
    put_u32(&mut superblock, 56, crc);
    dev.write(1, &superblock).unwrap();

    let mut fs = Fs::mount(dev).unwrap();
    assert_eq!(fs.version, 3);
    assert_eq!(fs.direct_blocks, MAX_DIRECT_BLOCKS);
    let ino = fs.create(b"/old", 1, INODE_FILE).unwrap();
    assert_eq!(
        fs.write(ino, 0, &[0x7a; 11 * BLOCK_SIZE], 1).unwrap(),
        11 * BLOCK_SIZE
    );
    fs.fsck().unwrap();
    let mut out = [0u8; 11 * BLOCK_SIZE];
    assert_eq!(fs.read(ino, 0, &mut out, 1).unwrap(), out.len());
    assert_eq!(out, [0x7a; 11 * BLOCK_SIZE]);
}

/// r299 (đỏ-trước): fsck/unlink/truncate trên ảnh VF02 (direct = 12)
/// index `blocks[12]` ngoài mảng [u32; 12] — panic trong VFS server ngay
/// lúc mount vì server gọi fsck ở boot.
#[test]
fn vf02_image_survives_fsck_truncate_and_unlink() {
    let mut dev = MemDev::new();
    format(&mut dev).unwrap();
    let mut superblock = zero_block();
    dev.read(1, &mut superblock).unwrap();
    superblock[..4].copy_from_slice(b"VF02");
    put_u32(&mut superblock, 4, 2);
    let crc = crc32(&superblock[..56]);
    put_u32(&mut superblock, 56, crc);
    dev.write(1, &superblock).unwrap();

    let mut fs = Fs::mount(dev).unwrap();
    assert_eq!(fs.version, 2);
    assert_eq!(fs.direct_blocks, MAX_BLOCKS_PER_INODE);
    let ino = fs.create(b"/legacy", 1, INODE_FILE).unwrap();
    assert_eq!(
        fs.write(ino, 0, &[0x5a; 12 * BLOCK_SIZE], 1).unwrap(),
        12 * BLOCK_SIZE
    );
    fs.fsck().unwrap();
    // truncate từ 12 block xuống 1 block phải đi qua trim_indirect.
    fs.truncate(ino, 100, 1).unwrap();
    fs.fsck().unwrap();
    // unlink đi qua free_inode_blocks.
    fs.unlink(b"/legacy", 1).unwrap();
    fs.fsck().unwrap();
    let mut reopened = reopen(&fs);
    reopened.fsck().unwrap();
}
