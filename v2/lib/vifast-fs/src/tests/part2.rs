// SPDX-License-Identifier: MIT OR Apache-2.0
// PORTED-FROM: crates/vifast-fs/src/lib.rs@ca2eaf5
use super::std;
use super::*;
use crate::tx::*;
/// r299 (đỏ-trước): truncate thư mục là mất dữ liệu toàn bộ namespace
/// con — phải chặn IsDir như read/write đã chặn.
#[test]
fn truncate_on_a_directory_is_rejected() {
    let mut fs = fresh();
    fs.mkdir(b"/d", 1).unwrap();
    fs.create(b"/d/inside", 1, INODE_FILE).unwrap();
    let dir = fs.lookup(b"/d", 1).unwrap();
    assert_eq!(fs.truncate(dir, 0, 1), Err(FsError::IsDir));
    // Không có gì biến mất.
    assert!(fs.lookup(b"/d/inside", 1).is_ok());
    fs.fsck().unwrap();
}

/// r299 (đỏ-trước): file thưa có lỗ ở vùng single/double là ảnh hợp lệ
/// do chính writer tạo ra — fsck không được báo Corrupt.
#[test]
fn fsck_accepts_sparse_files_with_indirect_holes() {
    let mut fs = fresh();
    let ino = fs.create(b"/sparse", 1, INODE_FILE).unwrap();
    // Lần ghi đầu rơi thẳng vào vùng double (block 150 > 10+128): cả
    // vùng single là lỗ.
    let offset = 150 * BLOCK_SIZE;
    assert_eq!(fs.write(ino, offset, &[0xAA; 16], 1).unwrap(), 16);
    // Lỗ giữa chừng trong vùng double: block 400 nhảy qua slot bảng.
    assert_eq!(fs.write(ino, 400 * BLOCK_SIZE, &[0xBB; 16], 1).unwrap(), 16);
    fs.fsck().unwrap();
    let mut reopened = reopen(&fs);
    reopened.fsck().unwrap();
    // Đọc lại qua lỗ phải ra số 0.
    let mut out = [0u8; 16];
    assert_eq!(fs.read(ino, 10 * BLOCK_SIZE, &mut out, 1).unwrap(), 16);
    assert_eq!(out, [0u8; 16]);
}

/// r299 (đỏ-trước): shrink 1024 -> 100 (old_size align block) phải xoá
/// đuôi block — grow lại rồi đọc cũng không thấy byte cũ.
#[test]
fn truncate_from_block_aligned_size_zeroes_the_partial_tail() {
    let mut fs = fresh();
    let ino = fs.create(b"/tail", 1, INODE_FILE).unwrap();
    assert_eq!(fs.write(ino, 0, &[0x7d; 2 * BLOCK_SIZE], 1).unwrap(), 1024);
    fs.truncate(ino, 100, 1).unwrap();
    // Grow lại bằng một ghi ở cuối: đọc block 0 phải thấy [0..100] giữ
    // nguyên và [100..512] là số 0 (POSIX: truncate+extend đọc được 0).
    assert_eq!(fs.write(ino, 1023, b"z", 1).unwrap(), 1);
    let mut out = [0u8; BLOCK_SIZE];
    assert_eq!(fs.read(ino, 0, &mut out, 1).unwrap(), BLOCK_SIZE);
    assert_eq!(&out[..100], &[0x7d; 100]);
    assert_eq!(&out[100..512], &[0u8; 412], "đuôi phải là số 0");
    fs.fsck().unwrap();
}

/// r299 (đỏ-trước): rename hỏng ở bước write_dir_entry (dir đích đầy)
/// không được để entry cũ biến mất theo transaction kế/fsync.
#[test]
fn failed_rename_rolls_back_the_removal() {
    let mut fs = fresh();
    fs.mkdir(b"/a", 1).unwrap();
    fs.mkdir(b"/b", 1).unwrap();
    fs.create(b"/a/f", 1, INODE_FILE).unwrap();
    // Lấp đầy /b tới mức dir_block_limit bằng hard-link.
    let filled = fill_dir_with_links(&mut fs, b"/b", b'f');
    assert!(filled > 0, "phải lấp được ít nhất một entry");
    assert_eq!(
        fs.rename(b"/a/f", b"/b/z", 1),
        Err(FsError::NoSpace),
        "dir đích đã đầy"
    );
    // Entry cũ phải còn nguyên.
    assert!(
        fs.lookup(b"/a/f", 1).is_ok(),
        "rename hỏng không được mất entry cũ"
    );
    // fsync không được commit removal đã stage.
    fs.fsync().unwrap();
    assert!(fs.lookup(b"/a/f", 1).is_ok());
    let mut reopened = reopen(&fs);
    assert!(reopened.lookup(b"/a/f", 1).is_ok());
    reopened.fsck().unwrap();
}

/// r299 (đỏ-trước): symlink hỏng ở write_dir_entry phải trả lại inode +
/// block đã cấp — không thì fsck báo inode mồ côi vĩnh viễn.
#[test]
fn failed_symlink_rolls_back_inode_and_block() {
    let mut fs = fresh();
    fs.mkdir(b"/b", 1).unwrap();
    let filled = fill_dir_with_links(&mut fs, b"/b", b's');
    assert!(filled > 0);
    assert_eq!(fs.symlink(b"/target", b"/b/sym", 1), Err(FsError::NoSpace));
    // Một transaction kế: rollback phải đủ sạch để fsck xanh.
    fs.create(b"/ok", 1, INODE_FILE).unwrap();
    fs.fsync().unwrap();
    fs.fsck().unwrap();
    let mut reopened = reopen(&fs);
    reopened.fsck().unwrap();
}

/// r299 (đỏ-trước): tên chứa NUL bị fsck từ chối (Corrupt) — writer
/// phải chặn ngay từ đầu, không tạo nổi ảnh dirty.
#[test]
fn names_with_nul_bytes_are_rejected() {
    let mut fs = fresh();
    assert_eq!(fs.create(b"/a\0b", 1, INODE_FILE), Err(FsError::Invalid));
    assert_eq!(fs.mkdir(b"/c\0d", 1), Err(FsError::Invalid));
    fs.create(b"/ok", 1, INODE_FILE).unwrap();
    fs.fsck().unwrap();
}

#[test]
fn crash_every_write_keeps_fs_consistent() {
    let baseline = fresh();
    // Determine the write count of one mutation.
    let mut probe = Fs::mount(CrashDev {
        data: baseline.dev.data.clone(),
        writes: 0,
        fail_at: usize::MAX,
        failed: false,
    })
    .unwrap();
    probe.mkdir(b"/crash", 0).unwrap();
    let total = probe.dev.writes;
    assert!(total > 3);
    for k in 0..=total {
        let dev = CrashDev {
            data: baseline.dev.data.clone(),
            writes: 0,
            fail_at: k,
            failed: false,
        };
        let mut fs = match Fs::mount(dev) {
            Ok(fs) => fs,
            Err(_) => continue,
        };
        let result = fs.mkdir(b"/crash", 0);
        assert!(result.is_err() || fs.lookup(b"/crash", 0).is_ok());
        let mut image = MemDev {
            data: fs.dev.data.clone(),
        };
        let mut mounted = match Fs::mount(image.clone()) {
            Ok(fs) => fs,
            Err(error) => panic!("mount failed at k={k}: {error:?}"),
        };
        mounted.fsck().unwrap();
        let _ = &mut image;
    }
}

#[test]
fn errors_and_limits() {
    let mut fs = fresh();
    assert_eq!(fs.lookup(b"/missing", 1), Err(FsError::NotFound));
    fs.create(b"/f", 1, INODE_FILE).unwrap();
    assert_eq!(fs.create(b"/f", 1, INODE_FILE), Err(FsError::Exists));
    let file = fs.lookup(b"/f", 1).unwrap();
    assert_eq!(fs.write(file, 0, b"x", 2), Err(FsError::NoAccess));
    assert_eq!(fs.lookup(b"/f/x", 1), Err(FsError::NotDir));
    assert_eq!(fs.read(ROOT_INO, 0, &mut [0u8; 1], 1), Err(FsError::IsDir));
    fs.mkdir(b"/d", 1).unwrap();
    fs.create(b"/d/inside", 1, INODE_FILE).unwrap();
    assert_eq!(fs.unlink(b"/d", 1), Err(FsError::NotEmpty));
    let long = [b'a'; MAX_NAME + 1];
    let mut path = [0u8; MAX_PATH];
    path[0] = b'/';
    path[1..1 + long.len()].copy_from_slice(&long);
    assert_eq!(
        fs.mkdir(&path[..1 + long.len()], 1),
        Err(FsError::NameTooLong)
    );
    let too_large = vec![0u8; MAX_FILE_SIZE + 1];
    assert_eq!(fs.write(file, 0, &too_large, 1), Err(FsError::NoSpace));
    let fits = vec![0u8; MAX_FILE_SIZE_V3];
    assert_eq!(fs.write(file, 0, &fits, 1).unwrap(), MAX_FILE_SIZE_V3);
    let mut made = 0usize;
    loop {
        let mut name = [0u8; 8];
        name[..3].copy_from_slice(b"/iX");
        name[2] = b'0' + (made % 10) as u8;
        name[3] = b'0' + ((made / 10) % 10) as u8;
        match fs.create(&name[..4], 1, INODE_FILE) {
            Ok(_) => made += 1,
            Err(FsError::NoInode) => break,
            Err(error) => panic!("unexpected {error:?}"),
        }
        assert!(made < MAX_INODES);
    }
    assert!(made > 0);
}

#[test]
fn legacy_migration() {
    let mut dev = MemDev::new();
    let nonce = b"legacy-nonce";
    dev.data[..4].copy_from_slice(b"VF01");
    dev.data[4..8].copy_from_slice(&(nonce.len() as u32).to_le_bytes());
    dev.data[8..8 + nonce.len()].copy_from_slice(nonce);
    let mut fs = Fs::mount(dev).unwrap();
    assert!(fs.migrated());
    let ino = fs.lookup(b"/scratch", 0).unwrap();
    let mut out = [0u8; 16];
    let len = fs.read(ino, 0, &mut out, 0).unwrap();
    assert_eq!(&out[..len], nonce);
    assert_eq!(&fs.dev.data[..4], &[0u8; 4]);
    fs.fsck().unwrap();
}

enum Op {
    Create,
    Write,
    Truncate,
    Unlink,
    Rename,
    Mkdir,
}

fn apply_op<D: BlockDev>(fs: &mut Fs<D>, op: &Op) -> Result<(), FsError> {
    match op {
        Op::Create => fs.create(b"/new", 1, INODE_FILE).map(|_| ()),
        Op::Write => {
            let ino = fs.lookup(b"/a", 1)?;
            fs.write(ino, 0, b"new-value", 1).map(|_| ())
        }
        Op::Truncate => {
            let ino = fs.lookup(b"/a", 1)?;
            fs.truncate(ino, 1, 1)
        }
        Op::Unlink => fs.unlink(b"/a", 1),
        Op::Rename => fs.rename(b"/a", b"/renamed", 1),
        Op::Mkdir => fs.mkdir(b"/newdir", 1).map(|_| ()),
    }
}

fn baseline_image() -> Vec<u8> {
    let mut fs = fresh();
    let a = fs.create(b"/a", 1, INODE_FILE).unwrap();
    fs.write(a, 0, b"old-value", 1).unwrap();
    fs.create(b"/b", 1, INODE_FILE).unwrap();
    fs.fsck().unwrap();
    fs.dev.data
}

#[test]
fn crash_every_write_for_mutations() {
    let baseline = baseline_image();
    for op in [
        Op::Create,
        Op::Write,
        Op::Truncate,
        Op::Unlink,
        Op::Rename,
        Op::Mkdir,
    ] {
        let mut probe = Fs::mount(CrashDev {
            data: baseline.clone(),
            writes: 0,
            fail_at: usize::MAX,
            failed: false,
        })
        .unwrap();
        let _ = apply_op(&mut probe, &op);
        let total = probe.dev.writes;
        for k in 0..=total {
            let mut fs = Fs::mount(CrashDev {
                data: baseline.clone(),
                writes: 0,
                fail_at: k,
                failed: false,
            })
            .unwrap();
            let _ = apply_op(&mut fs, &op);
            let image = fs.dev.data.clone();
            let mut mounted = Fs::mount(MemDev { data: image }).unwrap();
            mounted.fsck().unwrap_or_else(|e| {
                panic!(
                    "op={} k={k} fsck {e:?}",
                    match op {
                        Op::Create => "create",
                        Op::Write => "write",
                        Op::Truncate => "truncate",
                        Op::Unlink => "unlink",
                        Op::Rename => "rename",
                        Op::Mkdir => "mkdir",
                    }
                )
            });
            match op {
                Op::Write => {
                    let ino = mounted.lookup(b"/a", 1).unwrap();
                    let mut data = [0u8; 16];
                    let len = mounted.read(ino, 0, &mut data, 1).unwrap();
                    assert!(
                        len == 9 && &data[..len] == b"new-value"
                            || len == 9 && &data[..len] == b"old-value"
                    );
                }
                Op::Truncate => {
                    let ino = mounted.lookup(b"/a", 1).unwrap();
                    let size = mounted.info(ino).unwrap().size;
                    assert!(size == 9 || size == 1);
                }
                Op::Create => {
                    let exists = mounted.lookup(b"/new", 1).is_ok();
                    let _ = exists; // both states are valid after a crash
                }
                Op::Mkdir => {
                    let exists = mounted.lookup(b"/newdir", 1).is_ok();
                    let _ = exists;
                }
                Op::Rename => {
                    let a = mounted.lookup(b"/a", 1).is_ok();
                    let b = mounted.lookup(b"/renamed", 1).is_ok();
                    assert!((a && !b) || (!a && b));
                }
                Op::Unlink => {
                    let a = mounted.lookup(b"/a", 1);
                    assert!(a.is_ok() || a == Err(FsError::NotFound));
                }
            }
        }
    }
}

#[test]
fn random_2000_operations_with_checks() {
    let mut fs = fresh();
    let ino = fs.create(b"/rand", 1, INODE_FILE).unwrap();
    let mut model = vec![0u8; 1024];
    let mut model_len = 0usize;
    let mut seed = 0x1234_5678u32;
    let mut next = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed
    };
    for step in 0..2000usize {
        let op = next() % 3;
        let offset_limit = core::cmp::max(model_len, 1);
        let offset = if model_len == 0 {
            0
        } else {
            (next() as usize) % (model_len + 1)
        };
        let _ = offset_limit;
        let len = core::cmp::min((next() as usize) % 300, MAX_FILE_SIZE - offset);
        if op == 0 {
            let mut bytes = [0u8; 300];
            for byte in bytes[..len].iter_mut() {
                *byte = next() as u8;
            }
            if fs.write(ino, offset, &bytes[..len], 1).is_ok() {
                model[offset..offset + len].copy_from_slice(&bytes[..len]);
                model_len = model_len.max(offset + len);
            }
        } else if op == 1 {
            let new_len = (next() as usize) % 600;
            if fs.truncate(ino, new_len, 1).is_ok() {
                if new_len > model_len {
                    for byte in model[model_len..new_len].iter_mut() {
                        *byte = 0;
                    }
                }
                model_len = new_len;
            }
        } else {
            let mut out = [0u8; 300];
            let count = fs.read(ino, offset, &mut out[..len], 1).unwrap_or(0);
            if count != 0 {
                assert_eq!(&out[..count], &model[offset..offset + count]);
            }
        }
        if step % 100 == 0 {
            fs.fsck().unwrap_or_else(|e| panic!("step {step}: {e:?}"));
        }
        if step % 37 == 0 {
            // Crash the next mutation at a random write boundary and
            // require either the old or the fully applied model.
            let old_model = model.clone();
            let old_len = model_len;
            let mut bytes = [0u8; 64];
            for byte in bytes.iter_mut() {
                *byte = next() as u8;
            }
            let crash_at = (next() as usize) % 80;
            let mut crash = Fs::mount(CrashDev {
                data: fs.dev.data.clone(),
                writes: 0,
                fail_at: crash_at,
                failed: false,
            })
            .unwrap();
            let _result = crash.write(ino, 0, &bytes, 1);
            let image = crash.dev.data;
            let mut mounted = Fs::mount(MemDev { data: image }).unwrap();
            mounted.fsck().unwrap();
            let mut out = vec![0u8; 1024];
            let count = mounted.read(ino, 0, &mut out, 1).unwrap();
            let mut expected = old_model.clone();
            expected[..64].copy_from_slice(&bytes);
            let expected_len = old_len.max(64);
            let old_matches = count == old_len && out[..count] == old_model[..old_len];
            let new_matches = count == expected_len && out[..count] == expected[..expected_len];
            assert!(old_matches || new_matches, "crash produced a partial write");
            if new_matches {
                model = expected;
                model_len = expected_len;
            }
            fs = mounted;
        }
    }
    fs.fsck().unwrap();
}

fn small_fs(options: FormatOptions) -> Fs<MemDev> {
    let mut dev = MemDev::with_blocks(48);
    format_with_options(&mut dev, options).unwrap();
    Fs::mount(dev).unwrap()
}

#[test]
fn small_disk_enospc_then_free_and_reuse() {
    let mut fs = small_fs(FormatOptions {
        total_blocks: 48,
        inode_limit: 64,
        dir_block_limit: 11,
    });
    let full = fs.create(b"/full", 1, INODE_FILE).unwrap();
    assert_eq!(
        fs.write(full, 0, &[0x11; 12 * BLOCK_SIZE], 1).unwrap(),
        12 * BLOCK_SIZE
    );
    let rest = fs.create(b"/rest", 1, INODE_FILE).unwrap();
    assert_eq!(
        fs.write(rest, 0, &[0x22; 4 * BLOCK_SIZE], 1).unwrap(),
        4 * BLOCK_SIZE
    );
    let third = fs.create(b"/third", 1, INODE_FILE).unwrap();
    assert_eq!(
        fs.write(third, 0, &[0x33; BLOCK_SIZE], 1),
        Err(FsError::NoSpace)
    );
    let image = fs.dev.data.clone();
    let mut mounted = Fs::mount(MemDev { data: image }).unwrap();
    mounted.fsck().unwrap();
    assert_eq!(mounted.unlink(b"/full", 1), Ok(()));
    let again = mounted.create(b"/again", 1, INODE_FILE).unwrap();
    assert_eq!(
        mounted.write(again, 0, &[0x44; 8 * BLOCK_SIZE], 1).unwrap(),
        8 * BLOCK_SIZE
    );
    mounted.fsck().unwrap();
}

#[test]
fn one_block_directory_reaches_full_then_reuses_slot() {
    let mut fs = small_fs(FormatOptions {
        total_blocks: 48,
        inode_limit: 64,
        dir_block_limit: 1,
    });
    for index in 0..DIR_ENTRIES_PER_BLOCK {
        let mut path = [0u8; 5];
        path[..2].copy_from_slice(b"/f");
        path[2] = b'0' + (index / 10) as u8;
        path[3] = b'0' + (index % 10) as u8;
        fs.create(&path[..4], 1, INODE_FILE).unwrap();
    }
    let mut extra = [0u8; 5];
    extra[..2].copy_from_slice(b"/f");
    extra[2] = b'1';
    extra[3] = b'6';
    assert_eq!(fs.create(&extra[..4], 1, INODE_FILE), Err(FsError::NoSpace));
    let image = fs.dev.data.clone();
    let mut mounted = Fs::mount(MemDev { data: image }).unwrap();
    mounted.fsck().unwrap();
    assert_eq!(mounted.unlink(b"/f00", 1), Ok(()));
    mounted.create(b"/g", 1, INODE_FILE).unwrap();
    mounted.fsck().unwrap();
}

#[test]
fn crash_near_full_small_image_keeps_all_or_nothing() {
    let mut dev = MemDev::with_blocks(49);
    format_with_options(
        &mut dev,
        FormatOptions {
            total_blocks: 49,
            inode_limit: 64,
            dir_block_limit: 11,
        },
    )
    .unwrap();
    let mut fs = Fs::mount(dev).unwrap();
    let full = fs.create(b"/full", 1, INODE_FILE).unwrap();
    fs.write(full, 0, &[0x11; 8 * BLOCK_SIZE], 1).unwrap();
    let rest = fs.create(b"/rest", 1, INODE_FILE).unwrap();
    fs.write(rest, 0, &[0x22; 5 * BLOCK_SIZE], 1).unwrap();
    fs.create(b"/spare", 1, INODE_FILE).unwrap();
    fs.fsck().unwrap();
    let baseline = fs.dev.data.clone();
    let mut probe = Fs::mount(CrashDev {
        data: baseline.clone(),
        writes: 0,
        fail_at: usize::MAX,
        failed: false,
    })
    .unwrap();
    let spare = probe.lookup(b"/spare", 1).unwrap();
    probe.write(spare, 0, b"one", 1).unwrap();
    let total = probe.dev.writes;
    assert!(total >= 5);
    for k in 0..=total {
        let mut crashed = Fs::mount(CrashDev {
            data: baseline.clone(),
            writes: 0,
            fail_at: k,
            failed: false,
        })
        .unwrap();
        let spare = crashed.lookup(b"/spare", 1).unwrap();
        let _ = crashed.write(spare, 0, b"one", 1);
        let image = crashed.dev.data;
        let mut mounted = Fs::mount(MemDev { data: image }).unwrap();
        mounted.fsck().unwrap();
        let spare = mounted.lookup(b"/spare", 1).unwrap();
        let size = mounted.info(spare).unwrap().size;
        assert!(
            size == 0 || size == 3,
            "partial write at k={k}: size={size}"
        );
    }
}

#[test]
fn corrupt_dir_entry_length_does_not_panic() {
    // Vòng 83 (audit #2 P0-1): một byte độ dài tên hỏng trên đĩa từng làm
    // VFS server panic (slice index) → mọi IPC treo.
    let mut fs = fresh();
    let dir = fs.create(b"/d", 1, INODE_DIR).unwrap();
    fs.create(b"/d/a", 1, INODE_FILE).unwrap();
    let block_no = fs.inodes[dir as usize - 1].blocks[0];
    assert!(block_no != 0);
    let mut dev = fs.dev.clone();
    let off = block_no as usize * BLOCK_SIZE;
    dev.data[off + 2] = 200; // len rác, lớn hơn MAX_NAME
    let mut fs = Fs::mount(dev).unwrap();
    // Không được panic; entry hỏng bị bỏ qua.
    let _ = fs.lookup(b"/d/a", 0);
    let _ = fs.read_dir_entry(dir, 0);
}

#[test]
fn corrupt_dir_entry_keeps_readdir_indices_aligned() {
    // Vòng 88 (audit #4 P2): entry hỏng phải chiếm một index. Trước đây
    // [a, corrupt, c] đọc index 1 trả c và index 2 cũng trả c.
    let mut fs = fresh();
    let dir = fs.create(b"/d", 1, INODE_DIR).unwrap();
    fs.create(b"/d/a", 1, INODE_FILE).unwrap();
    fs.create(b"/d/b", 1, INODE_FILE).unwrap();
    fs.create(b"/d/c", 1, INODE_FILE).unwrap();
    let block_no = fs.inodes[dir as usize - 1].blocks[0];
    assert!(block_no != 0);
    let mut dev = fs.dev.clone();
    let off = block_no as usize * BLOCK_SIZE;
    // Slot 0 = "a"; làm hỏng trường độ dài của slot 1 ("b").
    dev.data[off + DIR_ENTRY_SIZE + 2] = 200;
    let mut fs = Fs::mount(dev).unwrap();
    let first = fs.read_dir_entry(dir, 0).unwrap().unwrap();
    assert_eq!(&first.1[..first.2], b"a");
    // Index 1 là entry hỏng: báo lỗi thay vì trả lại entry kế tiếp.
    assert_eq!(fs.read_dir_entry(dir, 1), Err(FsError::Corrupt));
    // Index 2 là "c", không phải "c" lặp lại.
    let third = fs.read_dir_entry(dir, 2).unwrap().unwrap();
    assert_eq!(&third.1[..third.2], b"c");
    // r296b: chỉ số là SLOT. Slot 3..7 nằm TRONG block nhưng trống ⇒ trả
    // bản ghi `ino == 0` (client đi tiếp), KHÔNG phải "hết". "Hết" chỉ khi
    // vượt quá slot cuối của block cuối (8 slot/block).
    for slot in 3..DIR_ENTRIES_PER_BLOCK {
        let empty = fs
            .read_dir_entry(dir, slot)
            .unwrap()
            .expect("ô trống vẫn là Some");
        assert_eq!(empty.0, 0, "slot {slot} phải là ô trống");
    }
    assert!(fs
        .read_dir_entry(dir, DIR_ENTRIES_PER_BLOCK)
        .unwrap()
        .is_none());
}

#[test]
fn corrupt_dir_size_beyond_block_slots_does_not_panic() {
    // Vòng 83 (audit #2 P0-2): dir size > 12 block làm index `blocks[]`
    // quá biên trong các vòng lặp thư mục.
    let mut fs = fresh();
    let dir = fs.create(b"/big", 1, INODE_DIR).unwrap();
    fs.inodes[dir as usize - 1].size = 7000;
    let _ = fs.read_dir_entry(dir, 0);
    let _ = fs.lookup(b"/big", 0);
    assert!(fs.inodes[dir as usize - 1].blocks_used() <= MAX_BLOCKS_PER_INODE);
}

#[test]
fn large_file_uses_single_indirect_and_frees_it_on_truncate() {
    let mut fs = fresh();
    let ino = fs.create(b"/large", 1, INODE_FILE).unwrap();
    let mut data = [0u8; 60 * 1024];
    for (index, byte) in data.iter_mut().enumerate() {
        *byte = (index * 31 + 7) as u8;
    }
    assert_eq!(fs.write(ino, 0, &data, 1).unwrap(), data.len());
    assert_eq!(fs.info(ino).unwrap().size as usize, data.len());
    {
        let inode = fs.inodes[ino as usize - 1];
        std::println!(
            "DEBUG size={} used={} direct={:?}",
            inode.size,
            inode.blocks_used(),
            inode.blocks
        );
        let ind = inode.blocks[V4_SINGLE_INDIRECT_INDEX];
        std::println!(
            "DEBUG indirect={} data0={} data10={} data11={}",
            ind,
            inode.blocks[0],
            inode.blocks[V4_DIRECT_BLOCKS],
            inode.blocks[V4_DOUBLE_INDIRECT_INDEX]
        );
        if ind != 0 {
            let mut b = zero_block();
            fs.dev.read(ind, &mut b).unwrap();
            std::println!(
                "DEBUG entries {} {} {}",
                get_u32(&b, 0),
                get_u32(&b, 4),
                get_u32(&b, 428)
            );
        }
    }
    fs.fsck().unwrap();
    let mut out = [0u8; 4096];
    let mut offset = 0usize;
    while offset < data.len() {
        let len = core::cmp::min(out.len(), data.len() - offset);
        assert_eq!(fs.read(ino, offset, &mut out[..len], 1).unwrap(), len);
        assert_eq!(&out[..len], &data[offset..offset + len]);
        offset += len;
    }
    fs.truncate(ino, 1000, 1).unwrap();
    assert_eq!(fs.info(ino).unwrap().size, 1000);
    fs.fsck().unwrap();
    let mut small = [0u8; 1000];
    assert_eq!(fs.read(ino, 0, &mut small, 1).unwrap(), 1000);
    assert_eq!(&small, &data[..1000]);
    fs.fsck().unwrap();
}

#[test]
fn double_indirect_grows_beyond_single_indirect() {
    let mut fs = fresh();
    let ino = fs.create(b"/double", 1, INODE_FILE).unwrap();
    let blocks = 200usize;
    let mut data = vec![0u8; blocks * BLOCK_SIZE];
    for (index, byte) in data.iter_mut().enumerate() {
        *byte = (index * 13 + 11) as u8;
    }
    assert_eq!(fs.write(ino, 0, &data, 1).unwrap(), data.len());
    let inode = fs.inodes[ino as usize - 1];
    assert_eq!(inode.blocks_used(), blocks);
    assert_ne!(inode.blocks[V4_SINGLE_INDIRECT_INDEX], 0);
    assert_ne!(inode.blocks[V4_DOUBLE_INDIRECT_INDEX], 0);
    fs.fsck().unwrap();

    let mut out = [0u8; 4096];
    let mut offset = 0usize;
    while offset < data.len() {
        let len = core::cmp::min(out.len(), data.len() - offset);
        assert_eq!(fs.read(ino, offset, &mut out[..len], 1).unwrap(), len);
        assert_eq!(&out[..len], &data[offset..offset + len]);
        offset += len;
    }

    fs.truncate(ino, 100 * BLOCK_SIZE, 1).unwrap();
    assert_eq!(
        fs.inodes[ino as usize - 1].blocks[V4_DOUBLE_INDIRECT_INDEX],
        0
    );
    fs.fsck().unwrap();
    fs.truncate(ino, 5 * BLOCK_SIZE, 1).unwrap();
    assert_eq!(
        fs.inodes[ino as usize - 1].blocks[V4_SINGLE_INDIRECT_INDEX],
        0
    );
    fs.fsck().unwrap();
}

#[test]
fn crash_during_first_indirect_block_allocation_keeps_fsck_clean() {
    let mut fs = fresh();
    let ino = fs.create(b"/indirect", 1, INODE_FILE).unwrap();
    fs.write(ino, 0, &[0x5a; 10 * BLOCK_SIZE], 1).unwrap();
    fs.fsck().unwrap();
    let baseline = fs.dev.data.clone();
    let mut probe = Fs::mount(CrashDev {
        data: baseline.clone(),
        writes: 0,
        fail_at: usize::MAX,
        failed: false,
    })
    .unwrap();
    probe
        .write(ino, 10 * BLOCK_SIZE, &[0xa5; BLOCK_SIZE], 1)
        .unwrap();
    let total = probe.dev.writes;
    assert!(total >= 5);
    for k in 0..=total {
        let mut crashed = Fs::mount(CrashDev {
            data: baseline.clone(),
            writes: 0,
            fail_at: k,
            failed: false,
        })
        .unwrap();
        let _ = crashed.write(ino, 10 * BLOCK_SIZE, &[0xa5; BLOCK_SIZE], 1);
        let image = crashed.dev.data;
        let mut mounted = Fs::mount(MemDev { data: image }).unwrap();
        mounted.fsck().unwrap();
        let size = mounted.info(ino).unwrap().size as usize;
        assert!(size == 10 * BLOCK_SIZE || size == 11 * BLOCK_SIZE);
    }
}

#[test]
fn symlink_follows_final_and_intermediate_components() {
    let mut fs = fresh();
    let target = fs.create(b"/target", 1, INODE_FILE).unwrap();
    fs.write(target, 0, b"symlink-data", 1).unwrap();
    fs.symlink(b"/target", b"/link", 1).unwrap();
    assert_eq!(fs.lookup(b"/link", 1).unwrap(), target);
    let (name, len) = fs.readlink(b"/link", 1).unwrap();
    assert_eq!(&name[..len], b"/target");
    fs.fsck().unwrap();

    fs.mkdir(b"/d", 1).unwrap();
    let inside = fs.create(b"/d/f", 1, INODE_FILE).unwrap();
    fs.symlink(b"/d", b"/dlink", 1).unwrap();
    assert_eq!(fs.lookup(b"/dlink/f", 1).unwrap(), inside);
    fs.symlink(b"d", b"/relative", 1).unwrap();
    assert_eq!(fs.lookup(b"/relative/f", 1).unwrap(), inside);
    fs.fsck().unwrap();

    fs.symlink(b"/missing", b"/dangling", 1).unwrap();
    assert_eq!(fs.lookup(b"/dangling", 1), Err(FsError::NotFound));
    fs.symlink(b"/loop", b"/loop", 1).unwrap();
    assert_eq!(fs.lookup(b"/loop", 1), Err(FsError::Invalid));
    fs.fsck().unwrap();

    fs.unlink(b"/link", 1).unwrap();
    assert_eq!(fs.lookup(b"/link", 1), Err(FsError::NotFound));
    assert_eq!(fs.lookup(b"/target", 1).unwrap(), target);
    fs.fsck().unwrap();
}

#[test]
fn symlink_rejects_empty_target_and_existing_name() {
    let mut fs = fresh();
    assert_eq!(fs.symlink(b"", b"/x", 1), Err(FsError::Invalid));
    fs.symlink(b"/target", b"/x", 1).unwrap();
    assert_eq!(fs.symlink(b"/other", b"/x", 1), Err(FsError::Exists));
    assert_eq!(fs.readlink(b"/x", 1).unwrap().1, 7);
    fs.fsck().unwrap();
}

#[test]
fn hard_link_keeps_data_until_last_name_is_removed() {
    let mut fs = fresh();
    let ino = fs.create(b"/a", 1, INODE_FILE).unwrap();
    let data = [0x5au8; 3 * BLOCK_SIZE];
    fs.write(ino, 0, &data, 1).unwrap();
    fs.link(b"/a", b"/b", 1).unwrap();
    fs.fsck().unwrap();
    assert_eq!(fs.info(ino).unwrap().nlink, 2);
    assert_eq!(fs.lookup(b"/b", 1).unwrap(), ino);
    let mut out = [0u8; 3 * BLOCK_SIZE];
    fs.read(ino, 0, &mut out, 1).unwrap();
    assert_eq!(out, data);
    fs.unlink(b"/a", 1).unwrap();
    fs.fsck().unwrap();
    assert_eq!(fs.info(ino).unwrap().nlink, 1);
    let mut out = [0u8; BLOCK_SIZE];
    let b = fs.lookup(b"/b", 1).unwrap();
    assert_eq!(fs.read(b, 0, &mut out, 1).unwrap(), BLOCK_SIZE);
    assert_eq!(&out, &data[..BLOCK_SIZE]);
    fs.unlink(b"/b", 1).unwrap();
    assert_eq!(fs.lookup(b"/b", 1), Err(FsError::NotFound));
    fs.fsck().unwrap();
}

#[test]
fn mount_rejects_device_smaller_than_superblock_needs() {
    // Vòng 166 (driver block): đĩa nhỏ hơn yêu cầu tối thiểu phải bị từ
    // chối ngay, không được panic hay ghi bừa ra ngoài phạm vi.
    let mut small = MemDev::with_blocks(DATA_START + 1);
    // format cần ít nhất DATA_START + 2 block (superblock + bitmap + dữ liệu).
    assert_eq!(format(&mut small), Err(FsError::NoSpace));
    assert!(Fs::mount(small).is_err());
}

#[test]
fn format_rejects_device_without_data_blocks() {
    // Đĩa chỉ đủ cho superblock + bitmap nhưng không có block dữ liệu.
    let mut tiny = MemDev::with_blocks(DATA_START);
    assert!(format(&mut tiny).is_err());
}

#[test]
fn writes_land_in_the_right_block_at_both_edges() {
    // Vòng 166: driver phải đúng ở hai biên — block 0 (superblock) và
    // block dữ liệu cuối cùng; ghi rồi đọc lại phải khớp.
    let mut dev = MemDev::new();
    format(&mut dev).unwrap();
    let last = dev.block_count() - 1;
    let mut pattern = [0u8; BLOCK_SIZE];
    for (i, byte) in pattern.iter_mut().enumerate() {
        *byte = (i as u8).wrapping_mul(7).wrapping_add(3);
    }
    dev.write(last, &pattern).unwrap();
    let mut back = [0u8; BLOCK_SIZE];
    dev.read(last, &mut back).unwrap();
    assert_eq!(back, pattern);
    // Superblock vẫn đọc được sau khi ghi block cuối.
    let mut sb = [0u8; BLOCK_SIZE];
    dev.read(1, &mut sb).unwrap();
    assert_eq!(&sb[0..4], b"VF04");
}

#[test]
fn root_is_empty_reports_user_entries_only() {
    // Vòng 163: quyết định "được phép format lại khi fsck lỗi" dựa vào
    // việc thư mục gốc có dữ liệu người dùng hay không.
    let mut fs = fresh();
    assert!(fs.root_is_empty(), "FS mới phải coi như trống");
    fs.mkdir(b"/d", 1).unwrap();
    assert!(!fs.root_is_empty(), "có thư mục con thì không còn trống");
    fs.unlink(b"/d", 1).unwrap();
    assert!(fs.root_is_empty(), "xoá hết thì trống lại");
    fs.create(b"/f", 1, INODE_FILE).unwrap();
    assert!(!fs.root_is_empty(), "có file thì không trống");
    fs.fsck().unwrap();
}

#[test]
fn hard_link_rejects_missing_existing_and_directory_targets() {
    let mut fs = fresh();
    assert_eq!(fs.link(b"/missing", b"/b", 1), Err(FsError::NotFound));
    fs.create(b"/a", 1, INODE_FILE).unwrap();
    fs.create(b"/b", 1, INODE_FILE).unwrap();
    assert_eq!(fs.link(b"/a", b"/b", 1), Err(FsError::Exists));
    fs.mkdir(b"/d", 1).unwrap();
    assert_eq!(fs.link(b"/d", b"/e", 1), Err(FsError::IsDir));
    fs.fsck().unwrap();
}

/// (#8) Batch-write regression: two 256B chunks in the same 512B block
/// (the `/prog.elf` staging pattern) must both land with the right size.
#[test]
fn issue8_batch_two_chunks_same_block() {
    let mut fs = fresh();
    let ino = fs.create(b"/a", 1, INODE_FILE).unwrap();
    let chunk1 = [0xAAu8; 256];
    let chunk2 = [0xBBu8; 256];
    // Exact root staging pattern: truncate on first open, then chunks.
    fs.truncate(ino, 0, 1).unwrap();
    assert_eq!(fs.write(ino, 0, &chunk1, 1).unwrap(), 256);
    assert_eq!(fs.write(ino, 256, &chunk2, 1).unwrap(), 256);
    assert_eq!(fs.info(ino).unwrap().size, 512);
    let mut out = [0u8; 512];
    let n = fs.read(ino, 0, &mut out, 1).unwrap();
    assert_eq!(n, 512);
    assert!(out[..256].iter().all(|&b| b == 0xAA));
    assert!(out[256..].iter().all(|&b| b == 0xBB));
    fs.fsck().unwrap();
}

#[test]
fn journal_txn_limit_coalesces_and_rejects_overflow() {
    // Mốc 3: txn journal tối đa 16 block — stage trùng LBA gộp (không
    // tốn slot), LBA thứ 17 bị TooManyWrites thay vì tràn journal.
    let mut fs = fresh();
    let filler = [0x5au8; BLOCK_SIZE];
    for lba in 30..30 + 16 {
        fs.stage(lba, &filler).unwrap();
    }
    // Stage lại LBA cũ: gộp, không tốn slot mới.
    fs.stage(30, &filler).unwrap();
    fs.stage(45, &filler).unwrap();
    // LBA mới thứ 17: từ chối.
    assert_eq!(fs.stage(100, &filler), Err(FsError::TooManyWrites));
    // Commit + fsck vẫn sạch (journal không hỏng).
    fs.commit().unwrap();
    fs.fsck().unwrap();
}

/// (#8) Table-driven CRC matches the bitwise oracle on the standard
/// vector ("123456789" -> 0xCBF43926) plus pseudo-random buffers, and
/// `crc32_continue` chains identically in both implementations.
#[test]
fn crc32_table_matches_bitwise_oracle() {
    assert_eq!(crc32(b"123456789"), 0xCBF43926);
    assert_eq!(crc32_bitwise(b"123456789"), 0xCBF43926);
    let mut buf = [0u8; 600];
    let mut x: u32 = 0x12345678;
    for slot in buf.iter_mut() {
        x = x.wrapping_mul(1664525).wrapping_add(1013904223);
        *slot = (x >> 24) as u8;
    }
    assert_eq!(crc32(&buf), crc32_bitwise(&buf));
    let (head, tail) = buf.split_at(211);
    let chained = crc32_continue(crc32(head), tail);
    let chained_ref = crc32_continue_bitwise(crc32_bitwise(head), tail);
    assert_eq!(chained, chained_ref);
    assert_eq!(chained, crc32(&buf));
}

#[test]
fn exec_reads_elf_from_fs_and_parses() {
    // Mốc 9 (vòng 18, bước 1): exec tương lai cần FS -> ELF bytes ->
    // parse. Viết 1 ELF64 tối giản vào FS, đọc ra qua read(), parse
    // bằng vifast-elf, kiểm tra entry + segment.
    let mut fs = fresh();
    // ELF64 LE exec, machine riscv (243), 1 PT_LOAD R+X.
    let mut elf = vec![0u8; 0x1000 + 64];
    elf[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    elf[4] = 2; // 64-bit
    elf[5] = 1; // LE
    elf[6] = 1; // version
    elf[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
    elf[18..20].copy_from_slice(&243u16.to_le_bytes()); // EM_RISCV
    elf[20..24].copy_from_slice(&1u32.to_le_bytes());
    elf[24..32].copy_from_slice(&0x10000u64.to_le_bytes()); // entry
    elf[32..40].copy_from_slice(&64u64.to_le_bytes()); // phoff
    elf[52..54].copy_from_slice(&64u16.to_le_bytes()); // ehsize
    elf[54..56].copy_from_slice(&56u16.to_le_bytes()); // phentsize
    elf[56..58].copy_from_slice(&1u16.to_le_bytes()); // phnum
    let ph = 64usize;
    elf[ph..ph + 4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
    elf[ph + 4..ph + 8].copy_from_slice(&5u32.to_le_bytes()); // R+X
    elf[ph + 8..ph + 16].copy_from_slice(&0x1000u64.to_le_bytes()); // offset
    elf[ph + 16..ph + 24].copy_from_slice(&0x10000u64.to_le_bytes()); // vaddr
    elf[ph + 24..ph + 32].copy_from_slice(&0x10000u64.to_le_bytes());
    elf[ph + 32..ph + 40].copy_from_slice(&64u64.to_le_bytes()); // filesz
    elf[ph + 40..ph + 48].copy_from_slice(&64u64.to_le_bytes()); // memsz
    elf[ph + 48..ph + 56].copy_from_slice(&0x1000u64.to_le_bytes()); // align
    for (i, byte) in elf.iter_mut().enumerate().skip(0x1000) {
        *byte = (i & 0xff) as u8;
    }
    let ino = fs.create(b"/init", 1, INODE_FILE).unwrap();
    fs.write(ino, 0, &elf, 1).unwrap();
    fs.fsck().unwrap();
    // Đọc lại qua FS read (nhiều chunk, qua biên block).
    let mut back = vec![0u8; elf.len()];
    let mut done = 0usize;
    while done < back.len() {
        let mut out = [0u8; BLOCK_SIZE];
        let n = fs.read(ino, done, &mut out, 1).unwrap();
        assert!(n > 0);
        back[done..done + n].copy_from_slice(&out[..n]);
        done += n;
    }
    assert_eq!(back, elf);
    // Parse bằng vifast-elf trong cửa sổ user riscv.
    let image = vifast_elf::parse(&back, 243, 0x10000, 0x10000 + 0x1000000).unwrap();
    assert_eq!(image.entry, 0x10000);
    assert_eq!(image.segment_count(), 1);
    let seg = image.segment(0).unwrap();
    assert!(seg.executable() && !seg.writable());
}
