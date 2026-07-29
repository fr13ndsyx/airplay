#![allow(clippy::all)]

use crate::consts::{
    INDEX_MANGLE, INITIAL_SESSION_KEY, MESSAGE_IV, MESSAGE_KEY, STATIC_SOURCE_1, STATIC_SOURCE_2,
    TABLE_S1, TABLE_S10, TABLE_S2, TABLE_S3, TABLE_S4, X_KEY, Z_KEY,
};

// 注意：default_sap 常量在 Rust 版中为 276 字节（足够覆盖 generate_session_key 中 oldSap[0x80..0x80+0x80] 的读取）。
pub fn decrypt_aes_key(message3: &[u8], cipher_text: &[u8], key_out: &mut [u8]) {
    let chunk1 = &cipher_text[16..];
    let chunk2 = &cipher_text[56..];

    let mut block_in = [0u8; 16];
    let mut sap_key = [0u8; 16];
    let mut key_schedule = [[0u32; 4]; 11];

    generate_session_key(&crate::consts::DEFAULT_SAP, message3, &mut sap_key);

    generate_key_schedule(&sap_key, &mut key_schedule);

    z_xor(chunk2, &mut block_in, 1);

    cycle(&mut block_in, &key_schedule);

    for i in 0..16 {
        key_out[i] = block_in[i] ^ chunk1[i];
    }

    // 注意：原地 XOR（输入==输出）在 Rust 借用检查下不允许同时可变+不可变借用。
    // 使用临时副本规避。
    {
        let mut tmp = [0u8; 16];
        tmp.copy_from_slice(&key_out[0..16]);
        x_xor(&tmp, key_out, 1);
    }

    {
        let mut tmp = [0u8; 16];
        tmp.copy_from_slice(&key_out[0..16]);
        z_xor(&tmp, key_out, 1);
    }
}

// For M0-M6 we follow the same pattern.
// 128 字节消息块解密，根据 mode（messageIn[12]）选择不同分支。
#[allow(clippy::too_many_lines)]
pub fn decrypt_message(message_in: &[u8], decrypted_message: &mut [u8]) {
    let mut buffer = [0u8; 16];
    let mut tmp: u8;

    // byte 是有符号的，但 0/1/2/3 都是正数，u8 直接对应
    let mode = message_in[12];

    // For M0-M6 we follow the same pattern
    for i in 0..8 {
        // First, copy in the nth block (we must start with the last one)
        for j in 0..16 {
            if mode == 3 {
                buffer[j] = message_in[(0x80 - 0x10 * i) + j];
            } else if mode == 2 || mode == 1 || mode == 0 {
                buffer[j] = message_in[(0x10 * (i + 1)) + j];
            }
        }
        // do this permutation and update 9 times. Could this be cycle(), or the reverse of cycle()?
        for j in 0..9 {
            let base = 0x80 - 0x10 * j;

            buffer[0x0] = message_table_index(base + 0x0)[buffer[0x0] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x0) as usize];
            buffer[0x4] = message_table_index(base + 0x4)[buffer[0x4] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x4) as usize];
            buffer[0x8] = message_table_index(base + 0x8)[buffer[0x8] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x8) as usize];
            buffer[0xc] = message_table_index(base + 0xc)[buffer[0xc] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0xc) as usize];

            tmp = buffer[0x0d];
            buffer[0xd] = message_table_index(base + 0xd)[buffer[0x9] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0xd) as usize];
            buffer[0x9] = message_table_index(base + 0x9)[buffer[0x5] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x9) as usize];
            buffer[0x5] = message_table_index(base + 0x5)[buffer[0x1] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x5) as usize];
            buffer[0x1] = message_table_index(base + 0x1)[tmp as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x1) as usize];

            tmp = buffer[0x02];
            buffer[0x2] = message_table_index(base + 0x2)[buffer[0xa] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x2) as usize];
            buffer[0xa] = message_table_index(base + 0xa)[tmp as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0xa) as usize];
            tmp = buffer[0x06];
            buffer[0x6] = message_table_index(base + 0x6)[buffer[0xe] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x6) as usize];
            buffer[0xe] = message_table_index(base + 0xe)[tmp as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0xe) as usize];

            tmp = buffer[0x3];
            buffer[0x3] = message_table_index(base + 0x3)[buffer[0x7] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x3) as usize];
            buffer[0x7] = message_table_index(base + 0x7)[buffer[0xb] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0x7) as usize];
            buffer[0xb] = message_table_index(base + 0xb)[buffer[0xf] as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0xb) as usize];
            buffer[0xf] = message_table_index(base + 0xf)[tmp as usize]
                ^ MESSAGE_KEY[mode as usize][(base + 0xf) as usize];

            // Now we must replace the entire buffer with 4 words that we read and xor together

            let table_s9 = crate::consts::table_s9();

            let word0 = table_s9[0x000 + buffer[0x0] as usize]
                ^ table_s9[0x100 + buffer[0x1] as usize]
                ^ table_s9[0x200 + buffer[0x2] as usize]
                ^ table_s9[0x300 + buffer[0x3] as usize];
            let word1 = table_s9[0x000 + buffer[0x4] as usize]
                ^ table_s9[0x100 + buffer[0x5] as usize]
                ^ table_s9[0x200 + buffer[0x6] as usize]
                ^ table_s9[0x300 + buffer[0x7] as usize];
            let word2 = table_s9[0x000 + buffer[0x8] as usize]
                ^ table_s9[0x100 + buffer[0x9] as usize]
                ^ table_s9[0x200 + buffer[0xa] as usize]
                ^ table_s9[0x300 + buffer[0xb] as usize];
            let word3 = table_s9[0x000 + buffer[0xc] as usize]
                ^ table_s9[0x100 + buffer[0xd] as usize]
                ^ table_s9[0x200 + buffer[0xe] as usize]
                ^ table_s9[0x300 + buffer[0xf] as usize];

            buffer[0x0..0x4].copy_from_slice(&word0.to_le_bytes());
            buffer[0x4..0x8].copy_from_slice(&word1.to_le_bytes());
            buffer[0x8..0xc].copy_from_slice(&word2.to_le_bytes());
            buffer[0xc..0x10].copy_from_slice(&word3.to_le_bytes());
        }
        // Next, another permute with a different table
        buffer[0x0] = TABLE_S10[((0x0 << 8) + buffer[0x0] as usize) as usize];
        buffer[0x4] = TABLE_S10[((0x4 << 8) + buffer[0x4] as usize) as usize];
        buffer[0x8] = TABLE_S10[((0x8 << 8) + buffer[0x8] as usize) as usize];
        buffer[0xc] = TABLE_S10[((0xc << 8) + buffer[0xc] as usize) as usize];

        tmp = buffer[0x0d];
        buffer[0xd] = TABLE_S10[((0xd << 8) + buffer[0x9] as usize) as usize];
        buffer[0x9] = TABLE_S10[((0x9 << 8) + buffer[0x5] as usize) as usize];
        buffer[0x5] = TABLE_S10[((0x5 << 8) + buffer[0x1] as usize) as usize];
        buffer[0x1] = TABLE_S10[((0x1 << 8) + tmp as usize) as usize];

        tmp = buffer[0x02];
        buffer[0x2] = TABLE_S10[((0x2 << 8) + buffer[0xa] as usize) as usize];
        buffer[0xa] = TABLE_S10[((0xa << 8) + tmp as usize) as usize];
        tmp = buffer[0x06];
        buffer[0x6] = TABLE_S10[((0x6 << 8) + buffer[0xe] as usize) as usize];
        buffer[0xe] = TABLE_S10[((0xe << 8) + tmp as usize) as usize];

        tmp = buffer[0x3];
        buffer[0x3] = TABLE_S10[((0x3 << 8) + buffer[0x7] as usize) as usize];
        buffer[0x7] = TABLE_S10[((0x7 << 8) + buffer[0xb] as usize) as usize];
        buffer[0xb] = TABLE_S10[((0xb << 8) + buffer[0xf] as usize) as usize];
        buffer[0xf] = TABLE_S10[((0xf << 8) + tmp as usize) as usize];

        // And finally xor with the previous block of the message, except in mode-2 where we do this in reverse
        let mut xor_result = [0u8; 16];
        if mode == 2 || mode == 1 || mode == 0 {
            if i > 0 {
                // remember that the first 0x10 bytes are the header
                xor_blocks(&buffer, &message_in[(0x10 * i)..(0x10 * i + 16)], &mut xor_result);
                decrypted_message[(0x10 * i)..(0x10 * i + 16)]
                    .copy_from_slice(&xor_result);
            } else {
                xor_blocks(&buffer, &MESSAGE_IV[mode as usize], &mut xor_result);
                decrypted_message[(0x10 * i)..(0x10 * i + 16)]
                    .copy_from_slice(&xor_result);
            }
        } else {
            if i < 7 {
                xor_blocks(
                    &buffer,
                    &message_in[(0x70 - 0x10 * i)..((0x70 - 0x10 * i) + 16)],
                    &mut xor_result,
                );
                decrypted_message[(0x70 - 0x10 * i)..((0x70 - 0x10 * i) + 16)]
                    .copy_from_slice(&xor_result);
            } else {
                xor_blocks(&buffer, &MESSAGE_IV[mode as usize], &mut xor_result);
                decrypted_message[(0x70 - 0x10 * i)..((0x70 - 0x10 * i) + 16)]
                    .copy_from_slice(&xor_result);
            }
        }
    }
}

// key_schedule 为 [11][4]，每轮 4 个 u32。
#[allow(clippy::too_many_lines)]
pub fn generate_key_schedule(key_material: &[u8], key_schedule: &mut [[u32; 4]; 11]) {
    let mut key_data = [0u32; 4];

    for i in 0..11 {
        key_schedule[i][0] = 0xdeadbeef;
        key_schedule[i][1] = 0xdeadbeef;
        key_schedule[i][2] = 0xdeadbeef;
        key_schedule[i][3] = 0xdeadbeef;
    }

    let mut buffer = [0u8; 16];
    let mut ti: i32 = 0;

    // G
    t_xor(key_material, &mut buffer);

    for i in 0..4 {
        key_data[i] = u32::from_le_bytes(buffer[i * 4..i * 4 + 4].try_into().unwrap());
    }

    for round in 0..11 {
        // H
        key_schedule[round][0] = key_data[0];

        // I
        let table1 = table_index(ti);
        let table2 = table_index(ti + 1);
        let table3 = table_index(ti + 2);
        let table4 = table_index(ti + 3);
        ti += 4;

        buffer[0] ^= table1[buffer[0x0d] as usize] ^ INDEX_MANGLE[round];
        buffer[1] ^= table2[buffer[0x0e] as usize];
        buffer[2] ^= table3[buffer[0x0f] as usize];
        buffer[3] ^= table4[buffer[0x0c] as usize];

        // H
        key_data[0] = u32::from_le_bytes(buffer[0..4].try_into().unwrap());

        // H
        key_schedule[round][1] = key_data[1];
        // J
        key_data[1] ^= key_data[0];
        buffer[4..8].copy_from_slice(&key_data[1].to_le_bytes());

        // H
        key_schedule[round][2] = key_data[2];
        // J
        key_data[2] ^= key_data[1];
        buffer[8..12].copy_from_slice(&key_data[2].to_le_bytes());

        // K and L
        // Implement K and L to fill in other bits of the key schedule
        key_schedule[round][3] = key_data[3];
        // J again
        key_data[3] ^= key_data[2];
        buffer[12..16].copy_from_slice(&key_data[3].to_le_bytes());
    }

    // 这段循环只是把 key_schedule 写入临时 tmp 数组再丢弃，
    // 实际上没有任何效果。为忠实翻译，这里保留空循环占位。
    // （wrap 被重新赋值指向 tmp，但 tmp 从未被读回，是死代码。）
    for _i in 0..11 {
        let _tmp = [0u8; 16];
        // （tmp 未被使用）
    }
}

// 生成会话密钥。5 轮 modifiedMD5 + sapHash。
#[allow(clippy::too_many_lines)]
pub fn generate_session_key(old_sap: &[u8], message_in: &[u8], session_key: &mut [u8]) {
    let mut decrypted_message = [0u8; 128];
    let mut new_sap = [0u8; 320];
    let mut md5 = [0u8; 16];

    decrypt_message(message_in, &mut decrypted_message);

    new_sap[0..0x11].copy_from_slice(&STATIC_SOURCE_1[0..0x11]);
    new_sap[0x11..0x11 + 0x80].copy_from_slice(&decrypted_message[0..0x80]);
    new_sap[0x091..0x091 + 0x80].copy_from_slice(&old_sap[0x80..0x80 + 0x80]);
    new_sap[0x111..0x111 + 0x2f].copy_from_slice(&STATIC_SOURCE_2[0..0x2f]);

    session_key[0..16].copy_from_slice(&INITIAL_SESSION_KEY[0..16]);

    for round in 0..5 {
        // 注意：这里是从 round*64 到末尾的切片，传给 modifiedMD5 和 sapHash。
        // modifiedMD5 内部只读前 64 字节，sapHash 内部按 (i % 64) 读取，
        // 所以切片长度对结果无影响，但必须从 round*64 开始。
        let base = &new_sap[(round * 64)..];

        crate::modified_md5::modified_md5(base, session_key, &mut md5);

        crate::sap_hash::sap_hash(base, session_key);

        for i in 0..4 {
            let sk = u32::from_le_bytes(session_key[i * 4..i * 4 + 4].try_into().unwrap());
            let md = u32::from_le_bytes(md5[i * 4..i * 4 + 4].try_into().unwrap());
            // 32 位无符号加法再截断
            let sum = sk.wrapping_add(md);
            session_key[i * 4..i * 4 + 4].copy_from_slice(&sum.to_le_bytes());
        }
    }

    // 即每个 4 字节组内部字节序反转（小端 ↔ 大端）
    let mut i = 0;
    while i < 16 {
        let tmp = session_key[i];
        session_key[i] = session_key[i + 3];
        session_key[i + 3] = tmp;
        let tmp = session_key[i + 1];
        session_key[i + 1] = session_key[i + 2];
        session_key[i + 2] = tmp;
        i += 4;
    }

    // Finally the whole thing is XORd with 121:
    for i in 0..16 {
        session_key[i] ^= 121;
    }
}

// 主加密循环。9 轮 T-table 查表 + XOR + permute。
#[allow(clippy::too_many_lines)]
pub fn cycle(block: &mut [u8], key_schedule: &[[u32; 4]; 11]) {
    let mut ptr1: u32;
    let mut ptr2: u32;
    let mut ptr3: u32;
    let mut ptr4: u32;
    let mut ab: u32;

    {
        let mut w0 = u32::from_le_bytes(block[0..4].try_into().unwrap());
        let mut w1 = u32::from_le_bytes(block[4..8].try_into().unwrap());
        let mut w2 = u32::from_le_bytes(block[8..12].try_into().unwrap());
        let mut w3 = u32::from_le_bytes(block[12..16].try_into().unwrap());
        w0 ^= key_schedule[10][0];
        w1 ^= key_schedule[10][1];
        w2 ^= key_schedule[10][2];
        w3 ^= key_schedule[10][3];
        block[0..4].copy_from_slice(&w0.to_le_bytes());
        block[4..8].copy_from_slice(&w1.to_le_bytes());
        block[8..12].copy_from_slice(&w2.to_le_bytes());
        block[12..16].copy_from_slice(&w3.to_le_bytes());
    }

    // First, these are permuted
    permute_block_1(block);

    let table_s5 = crate::consts::table_s5();
    let table_s6 = crate::consts::table_s6();
    let table_s7 = crate::consts::table_s7();
    let table_s8 = crate::consts::table_s8();

    for round in 0..9 {
        // E
        // Note that table_s5 is a table of 4-byte words. Therefore we do not need to <<2 these indices
        // TODO: Are these just T-tables?

        let mut key = [0u8; 16];
        for i in 0..4 {
            let v = key_schedule[9 - round][i];
            key[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }

        ptr1 = table_s5[(block[3] ^ key[3]) as usize];
        ptr2 = table_s6[(block[2] ^ key[2]) as usize];
        ptr3 = table_s8[(block[0] ^ key[0]) as usize];
        ptr4 = table_s7[(block[1] ^ key[1]) as usize];

        // A B
        ab = ptr1 ^ ptr2 ^ ptr3 ^ ptr4;

        // C
        block[0..4].copy_from_slice(&ab.to_le_bytes());

        ptr2 = table_s5[(block[7] ^ key[7]) as usize];
        ptr1 = table_s6[(block[6] ^ key[6]) as usize];
        ptr4 = table_s7[(block[5] ^ key[5]) as usize];
        ptr3 = table_s8[(block[4] ^ key[4]) as usize];

        // A B again
        ab = ptr1 ^ ptr2 ^ ptr3 ^ ptr4;

        // D is a bit of a nightmare, but it is really not as complicated as you might think
        block[4..8].copy_from_slice(&ab.to_le_bytes());

        let word2 = table_s5[(block[11] ^ key[11]) as usize]
            ^ table_s6[(block[10] ^ key[10]) as usize]
            ^ table_s7[(block[9] ^ key[9]) as usize]
            ^ table_s8[(block[8] ^ key[8]) as usize];
        block[8..12].copy_from_slice(&word2.to_le_bytes());

        let word3 = table_s5[(block[15] ^ key[15]) as usize]
            ^ table_s6[(block[14] ^ key[14]) as usize]
            ^ table_s7[(block[13] ^ key[13]) as usize]
            ^ table_s8[(block[12] ^ key[12]) as usize];
        block[12..16].copy_from_slice(&word3.to_le_bytes());

        // In the last round, instead of the permute, we do F
        permute_block_2(block, (8 - round) as i32);
    }

    {
        let mut w0 = u32::from_le_bytes(block[0..4].try_into().unwrap());
        let mut w1 = u32::from_le_bytes(block[4..8].try_into().unwrap());
        let mut w2 = u32::from_le_bytes(block[8..12].try_into().unwrap());
        let mut w3 = u32::from_le_bytes(block[12..16].try_into().unwrap());
        w0 ^= key_schedule[0][0];
        w1 ^= key_schedule[0][1];
        w2 ^= key_schedule[0][2];
        w3 ^= key_schedule[0][3];
        block[0..4].copy_from_slice(&w0.to_le_bytes());
        block[4..8].copy_from_slice(&w1.to_le_bytes());
        block[8..12].copy_from_slice(&w2.to_le_bytes());
        block[12..16].copy_from_slice(&w3.to_le_bytes());
    }
}

fn xor_blocks(a: &[u8], b: &[u8], out: &mut [u8]) {
    for i in 0..16 {
        out[i] = a[i] ^ b[i];
    }
}

fn z_xor(input: &[u8], output: &mut [u8], blocks: usize) {
    for j in 0..blocks {
        for i in 0..16 {
            output[j * 16 + i] = input[j * 16 + i] ^ Z_KEY[i];
        }
    }
}

fn x_xor(input: &[u8], output: &mut [u8], blocks: usize) {
    for j in 0..blocks {
        for i in 0..16 {
            output[j * 16 + i] = input[j * 16 + i] ^ X_KEY[i];
        }
    }
}

fn t_xor(input: &[u8], output: &mut [u8]) {
    for i in 0..16 {
        output[i] = input[i] ^ crate::consts::T_KEY[i];
    }
}

// 返回 table_s1 从 ((31 * i) % 0x28) << 8 开始到末尾的切片。
// 31 * i 可能溢出，按 i32 回绕语义处理。
fn table_index(i: i32) -> &'static [u8] {
    let start = (((31 * i) % 0x28) as usize) << 8;
    &TABLE_S1[start..]
}

// 注意运算符优先级：(97 * i) % 144。
fn message_table_index(i: i32) -> &'static [u8] {
    let start = (((97 * i) % 144) as usize) << 8;
    &TABLE_S2[start..]
}

fn permute_block_1(block: &mut [u8]) {
    block[0] = TABLE_S3[block[0] as usize];
    block[4] = TABLE_S3[0x400 + block[4] as usize];
    block[8] = TABLE_S3[0x800 + block[8] as usize];
    block[12] = TABLE_S3[0xc00 + block[12] as usize];

    let tmp = block[13];
    block[13] = TABLE_S3[0x100 + block[9] as usize];
    block[9] = TABLE_S3[0xd00 + block[5] as usize];
    block[5] = TABLE_S3[0x900 + block[1] as usize];
    block[1] = TABLE_S3[0x500 + tmp as usize];

    let tmp = block[2];
    block[2] = TABLE_S3[0xa00 + block[10] as usize];
    block[10] = TABLE_S3[0x200 + tmp as usize];
    let tmp = block[6];
    block[6] = TABLE_S3[0xe00 + block[14] as usize];
    block[14] = TABLE_S3[0x600 + tmp as usize];

    let tmp = block[3];
    block[3] = TABLE_S3[0xf00 + block[7] as usize];
    block[7] = TABLE_S3[0x300 + block[11] as usize];
    block[11] = TABLE_S3[0x700 + block[15] as usize];
    block[15] = TABLE_S3[0xb00 + tmp as usize];
}

fn permute_table_2(i: i32) -> &'static [u8] {
    let start = (((71 * i) % 144) as usize) << 8;
    &TABLE_S4[start..]
}

fn permute_block_2(block: &mut [u8], round: i32) {
    block[0] = permute_table_2(round * 16 + 0)[block[0] as usize];
    block[4] = permute_table_2(round * 16 + 4)[block[4] as usize];
    block[8] = permute_table_2(round * 16 + 8)[block[8] as usize];
    block[12] = permute_table_2(round * 16 + 12)[block[12] as usize];

    let tmp = block[13];
    block[13] = permute_table_2(round * 16 + 13)[block[9] as usize];
    block[9] = permute_table_2(round * 16 + 9)[block[5] as usize];
    block[5] = permute_table_2(round * 16 + 5)[block[1] as usize];
    block[1] = permute_table_2(round * 16 + 1)[tmp as usize];

    let tmp = block[2];
    block[2] = permute_table_2(round * 16 + 2)[block[10] as usize];
    block[10] = permute_table_2(round * 16 + 10)[tmp as usize];
    let tmp = block[6];
    block[6] = permute_table_2(round * 16 + 6)[block[14] as usize];
    block[14] = permute_table_2(round * 16 + 14)[tmp as usize];

    let tmp = block[3];
    block[3] = permute_table_2(round * 16 + 3)[block[7] as usize];
    block[7] = permute_table_2(round * 16 + 7)[block[11] as usize];
    block[11] = permute_table_2(round * 16 + 11)[block[15] as usize];
    block[15] = permute_table_2(round * 16 + 15)[tmp as usize];
}
