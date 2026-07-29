#![allow(clippy::all)]

const SHIFT: [u32; 64] = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
];

// MD5 F 函数：(B & C) | (~B & D)
fn f(b: u32, c: u32, d: u32) -> u32 {
    (b & c) | ((!b) & d)
}

// MD5 G 函数：(B & D) | (C & ~D)
fn g(b: u32, c: u32, d: u32) -> u32 {
    (b & d) | (c & (!d))
}

// MD5 H 函数：B ^ C ^ D
fn h(b: u32, c: u32, d: u32) -> u32 {
    b ^ c ^ d
}

// MD5 I 函数：C ^ (B | ~D)
// 注意：Rust snake_case 下函数名 i 会与循环变量 i 冲突（遮蔽），故命名为 ii。
fn ii(b: u32, c: u32, d: u32) -> u32 {
    c ^ (b | (!d))
}

// 32 位循环左移。等价于 u32::rotate_left。
fn rol(input: u32, count: u32) -> u32 {
    input.rotate_left(count)
}

// 以小端序交换 arr 中 idxA 与 idxB 处的 4 字节块。
fn swap(arr: &mut [u8], idx_a: usize, idx_b: usize) {
    let a = u32::from_le_bytes(arr[idx_a..idx_a + 4].try_into().unwrap());
    let b = u32::from_le_bytes(arr[idx_b..idx_b + 4].try_into().unwrap());
    arr[idx_b..idx_b + 4].copy_from_slice(&a.to_le_bytes());
    arr[idx_a..idx_a + 4].copy_from_slice(&b.to_le_bytes());
}

#[allow(clippy::too_many_lines)]
pub fn modified_md5(original_block_in: &[u8], key_in: &[u8], key_out: &mut [u8]) {
    let mut block_in: [u8; 64] = [0; 64];
    let mut a: u32;
    let mut b: u32;
    let mut c: u32;
    let mut d: u32;
    let mut z: u32;
    let mut tmp: u32;

    block_in[0..64].copy_from_slice(&original_block_in[0..64]);

    // Each cycle does something like this:
    a = u32::from_le_bytes(key_in[0..4].try_into().unwrap());
    b = u32::from_le_bytes(key_in[4..8].try_into().unwrap());
    c = u32::from_le_bytes(key_in[8..12].try_into().unwrap());
    d = u32::from_le_bytes(key_in[12..16].try_into().unwrap());

    for i in 0..64usize {
        let input: u32;
        let mut j: usize = 0;
        if i < 16 {
            j = i;
        } else if i < 32 {
            j = (5 * i + 1) % 16;
        } else if i < 48 {
            j = (3 * i + 5) % 16;
        } else if i < 64 {
            j = 7 * i % 16;
        }

        input = (((block_in[4 * j] as u32) & 0xFF) << 24)
            | (((block_in[4 * j + 1] as u32) & 0xFF) << 16)
            | (((block_in[4 * j + 2] as u32) & 0xFF) << 8)
            | ((block_in[4 * j + 3] as u32) & 0xFF);

        // 用 u64 计算中间值（加法可能超出 32 位），然后截断到 u32。
        let sin_term: u64 = (4294967296.0_f64 * (i as f64 + 1.0).sin().abs()) as u64;
        let z_u64: u64 = (a as u64).wrapping_add(input as u64).wrapping_add(sin_term);
        z = z_u64 as u32;

        if i < 16 {
            z = rol(z.wrapping_add(f(b, c, d)), SHIFT[i]);
        } else if i < 32 {
            z = rol(z.wrapping_add(g(b, c, d)), SHIFT[i]);
        } else if i < 48 {
            z = rol(z.wrapping_add(h(b, c, d)), SHIFT[i]);
        } else if i < 64 {
            z = rol(z.wrapping_add(ii(b, c, d)), SHIFT[i]);
        }

        z = z.wrapping_add(b);
        tmp = d;
        d = c;
        c = b;
        b = z;
        a = tmp;

        if i == 31 {
            // swapsies
            swap(&mut block_in, 4 * ((a & 15) as usize), 4 * ((b & 15) as usize));
            swap(&mut block_in, 4 * ((c & 15) as usize), 4 * ((d & 15) as usize));
            swap(
                &mut block_in,
                4 * (((a & (15 << 4)) >> 4) as usize),
                4 * (((b & (15 << 4)) >> 4) as usize),
            );
            swap(
                &mut block_in,
                4 * (((a & (15 << 8)) >> 8) as usize),
                4 * (((b & (15 << 8)) >> 8) as usize),
            );
            swap(
                &mut block_in,
                4 * (((a & (15 << 12)) >> 12) as usize),
                4 * (((b & (15 << 12)) >> 12) as usize),
            );
        }
    }

    let key_in_0 = u32::from_le_bytes(key_in[0..4].try_into().unwrap());
    let key_in_4 = u32::from_le_bytes(key_in[4..8].try_into().unwrap());
    let key_in_8 = u32::from_le_bytes(key_in[8..12].try_into().unwrap());
    let key_in_12 = u32::from_le_bytes(key_in[12..16].try_into().unwrap());
    key_out[0..4].copy_from_slice(&key_in_0.wrapping_add(a).to_le_bytes());
    key_out[4..8].copy_from_slice(&key_in_4.wrapping_add(b).to_le_bytes());
    key_out[8..12].copy_from_slice(&key_in_8.wrapping_add(c).to_le_bytes());
    key_out[12..16].copy_from_slice(&key_in_12.wrapping_add(d).to_le_bytes());
}
