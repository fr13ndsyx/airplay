#![allow(clippy::all)]

// 8 位循环左移。等价于 u8::rotate_left。
fn rol8(input: u8, count: u32) -> u8 {
    input.rotate_left(count)
}

// 扩展循环左移，返回 int（不截断到 byte）。
fn rol8x(input: u32, count: u32) -> u32 {
    (input << count) | (input >> (8 - count))
}

// 特殊：count == 0 时返回 0（而非 input），不能优化为标准 rotate。
fn weird_ror8(input: u32, count: u32) -> u32 {
    if count == 0 {
        return 0;
    }
    ((input >> count) & 0xff) | ((input & 0xff) << (8 - count))
}

// 特殊：count == 0 时返回 0。
fn weird_rol8(input: u32, count: u32) -> u32 {
    if count == 0 {
        return 0;
    }
    ((input << count) & 0xff) | ((input & 0xff) >> (8 - count))
}

// 特殊：count == 0 时返回 0，且用 ^（XOR）而非 |（OR）合并两半。
fn weird_rol32(input: u32, count: u32) -> u32 {
    if count == 0 {
        return 0;
    }
    (input << count) ^ (input >> (8 - count))
}

// I do not know why it is doing all of this, and there is still a possibility for a gremlin or two to be lurking in the background
// I DO know it is not trivial. It could be purely random garbage, of course.
#[allow(clippy::too_many_lines)]
pub fn garble(buffer0: &mut [u8], buffer1: &mut [u8], buffer2: &mut [u8], buffer3: &mut [u8], buffer4: &mut [u8]) {
    let mut tmp: u32;
    let mut tmp2: u32;
    let mut tmp3: u32;
    let mut a: u32;
    let mut b: u32;
    let mut c: u32;
    let mut d: u32;
    let mut e: u32;
    let mut m: u32;
    let mut j: u32;
    let mut g: u32;
    let mut f: u32;
    let mut h: u32;
    let mut k: u32;
    let mut r: u32;
    let mut s: u32;
    let mut t: u32;
    let mut u: u32;
    let mut v: u32;
    let mut w: u32;
    let mut x: u32;
    let mut y: u32;
    let mut z: u32;

    // buffer1[64] = A
    // (buffer1[99] / 3) = B
    // 0ABAAABB
    // Then we AND with a complex expression, and add 20 just for good measure
    buffer2[12] = (0x14u32
        .wrapping_add((((buffer1[64] as u32) & 92) | (((buffer1[99] as u32) / 3) & 35))
           & (buffer4[(rol8x(buffer4[(buffer1[206] as usize) % 21] as u32, 4) as usize % 21)] as u32)))
        as u8;

    // This is a bit simpler: 2*B*B/25
    buffer1[4] = (((buffer1[99] as u32) / 5).wrapping_mul((buffer1[99] as u32) / 5).wrapping_mul(2)) as u8;

    // Simpler still!
    buffer2[34] = 0xb8;

    // ...
    buffer1[153] ^= ((buffer2[(buffer1[203] as usize) % 35] as u32)
        .wrapping_mul(buffer2[(buffer1[203] as usize) % 35] as u32)
        .wrapping_mul(buffer1[190] as u32)) as u8;

    // This one looks simple, but wow was it not :(
    // 注意：字节右移需符号扩展。
    buffer0[3] = buffer0[3].wrapping_sub(
        ((((buffer4[(buffer1[205] as usize) % 21] as i8 as i32 >> 1) as u32 & 80) | 0xe6440) as u8),
    );

    // This is always 0x93
    buffer0[16] = 0x93;

    // This is always 0x62
    buffer0[13] = 0x62;

    buffer1[33] = buffer1[33].wrapping_sub((buffer4[(buffer1[36] as usize) % 21] as u32 & 0xf6) as u8);

    // This is always 7
    tmp2 = buffer2[(buffer1[67] as usize) % 35] as u32;
    buffer2[12] = 0x07;

    // This is pretty easy!
    tmp = buffer0[(buffer1[181] as usize) % 20] as u32;
    buffer1[2] = buffer1[2].wrapping_sub(3136u32 as u8);

    buffer0[19] = buffer4[(buffer1[58] as usize) % 21];

    buffer3[0] = (92u32.wrapping_sub(buffer2[(buffer1[32] as usize) % 35] as u32)) as u8;

    buffer3[4] = (buffer2[(buffer1[15] as usize) % 35] as u32).wrapping_add(0x9e) as u8;

    buffer1[34] = buffer1[34].wrapping_add(
        (buffer4[(((buffer2[(buffer1[15] as usize) % 35] as u32).wrapping_add(0x9e) & 0xff) as usize) % 21] as u32 / 5) as u8,
    );

    buffer0[19] = ((buffer0[19] as u32)
        .wrapping_add(0xfffffee6)
        .wrapping_sub(((buffer0[(buffer3[4] as usize) % 20] as u32 >> 1) & 102))) as u8;

    // This LOOKS like it should be a rol8x, but it just doesnt work out because if the shift amount is 0, then the output is 0 too :(
    // FIXME: Switch to weird_ror8
    buffer1[15] = ((3u32
        .wrapping_mul(
            ((((buffer1[72] as u32) >> ((buffer4[(buffer1[190] as usize) % 21] as u32) & 7))
                ^ ((buffer1[72] as u32) << ((7u32.wrapping_sub((buffer4[(buffer1[190] as usize) % 21] as u32).wrapping_sub(1))) & 7)))
                .wrapping_sub(3u32.wrapping_mul(buffer4[(buffer1[126] as usize) % 21] as u32))),
        ))
        ^ buffer1[15] as u32) as u8;

    buffer0[15] ^= ((buffer2[(buffer1[181] as usize) % 35] as u32)
        .wrapping_mul(buffer2[(buffer1[181] as usize) % 35] as u32)
        .wrapping_mul(buffer2[(buffer1[181] as usize) % 35] as u32)) as u8;

    buffer2[4] ^= (buffer1[202] as u32 / 3) as u8;

    // This could probably be quite a bit simpler.
    a = 92u32.wrapping_sub(buffer0[(buffer3[0] as usize) % 20] as u32);
    e = (a & 0xc6) | ((!(buffer1[105] as u32)) & 0xc6) | (a & (!(buffer1[105] as u32)));
    buffer2[1] = buffer2[1].wrapping_add((e.wrapping_mul(e).wrapping_mul(e)) as u8);

    buffer0[19] ^= (((224u32 | (buffer4[(buffer1[92] as usize) % 21] as u32 & 27))
        .wrapping_mul(buffer2[(buffer1[41] as usize) % 35] as u32))
        / 3) as u8;

    buffer1[140] = buffer1[140].wrapping_add(weird_ror8(92, (buffer1[5] as u32) & 7) as u8);

    // Is this as simple as it could be?
    buffer2[12] = buffer2[12].wrapping_add(
        (((((!(buffer1[4] as u32)) ^ buffer2[(buffer1[12] as usize) % 35] as u32) | (buffer1[182] as u32)) & 192)
            | (((!(buffer1[4] as u32)) ^ buffer2[(buffer1[12] as usize) % 35] as u32) & (buffer1[182] as u32))) as u8,
    );

    buffer1[36] = buffer1[36].wrapping_add(125);

    buffer1[124] = (rol8x(
        (((74u32 & (buffer1[138] as u32)) | ((74u32 | (buffer1[138] as u32)) & (buffer0[15] as u32)))
            & (buffer0[(buffer1[43] as usize) % 20] as u32))
            | (((74u32 & (buffer1[138] as u32))
                | ((74u32 | (buffer1[138] as u32)) & (buffer0[15] as u32))
                | (buffer0[(buffer1[43] as usize) % 20] as u32))
                & 95),
        4,
    )) as u8;

    buffer3[8] = (((((buffer0[(buffer3[4] as usize) % 20] as u32) & 95)
        & (((buffer4[(buffer1[68] as usize) % 21] as u32) & 46) << 1))
        | 16)
        ^ 92) as u8;

    a = (buffer1[177] as u32).wrapping_add(buffer4[(buffer1[79] as usize) % 21] as u32);
    d = (((a >> 1) | ((3u32.wrapping_mul(buffer1[148] as u32)) / 5)) & (buffer2[1] as u32))
        | ((a >> 1) & ((3u32.wrapping_mul(buffer1[148] as u32)) / 5));
    buffer3[12] = (0u32.wrapping_sub(34).wrapping_sub(d)) as u8;

    a = 8u32.wrapping_sub((buffer2[22] as u32) & 7); // FIXME: buffer2[22] = 74, so A is always 6 and B^C is just ror8(buffer1[33], 6)
    b = (buffer1[33] as u32) >> (a & 7);
    c = (buffer1[33] as u32) << ((buffer2[22] as u32) & 7);
    buffer2[16] = buffer2[16].wrapping_add(
        (((buffer2[(buffer3[0] as usize) % 35] as u32) & 159)
            | (buffer0[(buffer3[4] as usize) % 20] as u32)
            | 8)
            .wrapping_sub((b ^ c) | 128) as u8,
    );

    // This one was very easy so I just skipped ahead and did it
    buffer0[14] ^= buffer2[(buffer3[12] as usize) % 35];

    // Monster goes here
    a = weird_rol8(
        buffer4[(buffer0[(buffer1[201] as usize) % 20] as usize) % 21] as u32,
        ((buffer2[(buffer1[112] as usize) % 35] as u32) << 1) & 7,
    );
    d = (buffer0[(buffer1[208] as usize) % 20] as u32 & 131)
        | ((buffer0[(buffer1[164] as usize) % 20] as u32) & 124);
    buffer1[19] = buffer1[19].wrapping_add(
        ((a & (d / 5)) | ((a | (d / 5)) & 37)) as u8,
    );

    buffer2[8] = (weird_ror8(
        140,
        ((buffer4[(buffer1[45] as usize) % 21] as u32).wrapping_add(92).wrapping_mul((buffer4[(buffer1[45] as usize) % 21] as u32).wrapping_add(92))) & 7,
    ) & 0xff) as u8;

    buffer1[190] = 56;

    buffer2[8] ^= buffer3[0];

    buffer1[53] = (!(((buffer0[(buffer1[83] as usize) % 20] as u32) | 204) / 5)) as u8;

    buffer0[13] = buffer0[13].wrapping_add(buffer0[(buffer1[41] as usize) % 20]);

    buffer0[10] = ((((buffer2[(buffer3[0] as usize) % 35] as u32) & (buffer1[2] as u32))
        | (((buffer2[(buffer3[0] as usize) % 35] as u32) | (buffer1[2] as u32)) & (buffer3[12] as u32)))
        / 15) as u8;

    a = (((56u32 | ((buffer4[(buffer1[2] as usize) % 21] as u32) & 68)) | (buffer2[(buffer3[8] as usize) % 35] as u32)) & 42)
        | ((((buffer4[(buffer1[2] as usize) % 21] as u32) & 68) | 56) & (buffer2[(buffer3[8] as usize) % 35] as u32));
    buffer3[16] = (a.wrapping_mul(a).wrapping_add(110)) as u8;

    buffer3[20] = (202u32.wrapping_sub(buffer3[16] as u32)) as u8;

    buffer3[24] = buffer1[151];

    buffer2[13] ^= buffer4[(buffer3[0] as usize) % 21];

    b = (((buffer2[(buffer1[179] as usize) % 35] as u32).wrapping_sub(38)) & 177) | ((buffer3[12] as u32) & 177);
    c = ((buffer2[(buffer1[179] as usize) % 35] as u32).wrapping_sub(38)) & (buffer3[12] as u32);
    buffer3[28] = (30u32.wrapping_add((b | c).wrapping_mul(b | c))) as u8;

    buffer3[32] = (buffer3[28] as u32).wrapping_add(62) as u8;

    // eek
    a = (((buffer3[20] as u32).wrapping_add((buffer3[0] as u32) & 74) | !(buffer4[(buffer3[0] as usize) % 21] as u32)) & 121);
    b = ((buffer3[20] as u32).wrapping_add((buffer3[0] as u32) & 74) & !(buffer4[(buffer3[0] as usize) % 21] as u32));
    tmp3 = a | b;
    c = ((((a | b) ^ 0xffffffa6) | (buffer3[0] as u32)) & 4) | (((a | b) ^ 0xffffffa6) & (buffer3[0] as u32));
    buffer1[47] = ((buffer2[(buffer1[89] as usize) % 35] as u32).wrapping_add(c) ^ (buffer1[47] as u32)) as u8;

    buffer3[36] = (((rol8(((tmp & 179).wrapping_add(68)) as u8, 2) as u32 & (buffer0[3] as u32))
        | (tmp2 & !(buffer0[3] as u32)))
        .wrapping_sub(15)) as u8;

    buffer1[123] ^= 221;

    a = ((buffer4[(buffer3[0] as usize) % 21] as u32) / 3).wrapping_sub(buffer2[(buffer3[4] as usize) % 35] as u32);
    c = (((buffer3[0] as u32 & 163) + 92) & 246) | (buffer3[0] as u32 & 92);
    e = ((c | (buffer3[24] as u32)) & 54) | (c & (buffer3[24] as u32));
    buffer3[40] = (a.wrapping_sub(e)) as u8;

    buffer3[44] = (tmp3 ^ 81 ^ ((((buffer3[0] as u32) >> 1) & 101) + 26)) as u8;

    buffer3[48] = (buffer2[(buffer3[4] as usize) % 35] as u32 & 27) as u8;

    buffer3[52] = 27;

    buffer3[56] = 199;

    // caffeine
    buffer3[64] = ((buffer3[4] as u32)
        + ((((((((buffer3[40] as u32) | (buffer3[24] as u32)) & 177) | ((buffer3[40] as u32) & (buffer3[24] as u32)))
            & (((((buffer4[(buffer3[0] as usize) % 20] as u32) & 177) | 176)) | ((buffer4[(buffer3[0] as usize) % 21] as u32) & !3)))
            | (((((buffer3[40] as u32) & (buffer3[24] as u32)) | (((buffer3[40] as u32) | (buffer3[24] as u32)) & 177)) & 199)
                | ((((((buffer4[(buffer3[0] as usize) % 21] as u32) & 1) & 0xff) + 176) | ((buffer4[(buffer3[0] as usize) % 21] as u32) & !3))
                    & (buffer3[56] as u32))))
            & (!(buffer3[52] as u32)))
            | (buffer3[48] as u32))) as u8;

    buffer2[33] ^= buffer1[26];

    buffer1[106] ^= buffer3[20] ^ 133;

    buffer2[30] = ((((buffer3[64] as u32) / 3).wrapping_sub(275u32 | ((buffer3[0] as u32) & 247)))
        ^ (buffer0[(buffer1[122] as usize) % 20] as u32)) as u8;

    buffer1[22] = ((buffer2[(buffer1[90] as usize) % 35] as u32) & 95 | 68) as u8;

    a = ((buffer4[(buffer3[36] as usize) % 21] as u32) & 184) | ((buffer2[(buffer3[44] as usize) % 35] as u32) & !184);
    buffer2[18] = buffer2[18].wrapping_add((a.wrapping_mul(a).wrapping_mul(a) >> 1) as u8);

    buffer2[5] = buffer2[5].wrapping_sub(buffer4[(buffer1[92] as usize) % 21]);

    a = ((((buffer1[41] as u32) & !24) | ((buffer2[(buffer1[183] as usize) % 35] as u32) & 24)) & ((buffer3[16] as u32) + 53))
        | ((buffer3[20] as u32) & (buffer2[(buffer3[20] as usize) % 35] as u32));
    b = ((buffer1[17] as u32) & (!(buffer3[44] as u32))) | ((buffer0[(buffer1[59] as usize) % 20] as u32) & (buffer3[44] as u32));
    buffer2[18] ^= (a.wrapping_mul(b)) as u8;

    a = weird_ror8(buffer1[11] as u32, (buffer2[(buffer1[28] as usize) % 35] as u32) & 7) & 7;
    b = ((((buffer0[(buffer1[93] as usize) % 20] as u32) & !(buffer0[14] as u32)) | ((buffer0[14] as u32) & 150)) & !28)
        | ((buffer1[7] as u32) & 28);
    buffer2[22] = (((((b | weird_rol8(buffer2[(buffer3[0] as usize) % 35] as u32, a)) & (buffer2[33] as u32))
        | (b & weird_rol8(buffer2[(buffer3[0] as usize) % 35] as u32, a)))
        + 74)
        & 0xff) as u8;

    a = buffer4[((buffer0[(buffer1[39] as usize) % 20] as u32) ^ 217) as usize % 21] as u32;
    buffer0[15] = buffer0[15].wrapping_sub(
        ((((((buffer3[20] as u32) | (buffer3[0] as u32)) & 214) | ((buffer3[20] as u32) & (buffer3[0] as u32))) & a)
            | (((((buffer3[20] as u32) | (buffer3[0] as u32)) & 214) | ((buffer3[20] as u32) & (buffer3[0] as u32)) | a) & (buffer3[32] as u32))) as u8,
    );

    // We need to save T here, and boy is it complicated to calculate!
    b = (((buffer2[(buffer1[57] as usize) % 35] as u32 & buffer0[(buffer3[64] as usize) % 20] as u32)
        | ((buffer0[(buffer3[64] as usize) % 20] as u32 | buffer2[(buffer1[57] as usize) % 35] as u32) & 95)
        | (buffer3[64] as u32 & 45)
        | 82) & 32);
    c = ((buffer2[(buffer1[57] as usize) % 35] as u32 & buffer0[(buffer3[64] as usize) % 20] as u32)
        | ((buffer2[(buffer1[57] as usize) % 35] as u32 | buffer0[(buffer3[64] as usize) % 20] as u32) & 95))
        & ((buffer3[64] as u32 & 45) | 82);
    d = ((((buffer3[0] as u32) / 3).wrapping_sub((buffer3[64] as u32) | (buffer1[22] as u32)))
        ^ ((buffer3[28] as u32) + 62)
        ^ (b | c));
    t = buffer0[(d & 0xff) as usize % 20] as u32;

    buffer3[68] = (((buffer0[(buffer1[99] as usize) % 20] as u32)
        .wrapping_mul(buffer0[(buffer1[99] as usize) % 20] as u32)
        .wrapping_mul(buffer0[(buffer1[99] as usize) % 20] as u32)
        .wrapping_mul(buffer0[(buffer1[99] as usize) % 20] as u32))
        | (buffer2[(buffer3[64] as usize) % 35] as u32)) as u8;

    u = buffer0[(buffer1[50] as usize) % 20] as u32; // this is also v100
    w = buffer2[(buffer1[138] as usize) % 35] as u32;
    x = buffer4[(buffer1[39] as usize) % 21] as u32;
    y = buffer0[(buffer1[4] as usize) % 20] as u32; // this is also v120
    z = buffer4[(buffer1[202] as usize) % 21] as u32; // also v124
    v = buffer0[(buffer1[151] as usize) % 20] as u32;
    s = buffer2[(buffer1[14] as usize) % 35] as u32;
    r = buffer0[(buffer1[145] as usize) % 20] as u32;

    a = ((buffer2[(buffer3[68] as usize) % 35] as u32) & (buffer0[(buffer1[209] as usize) % 20] as u32))
        | (((buffer2[(buffer3[68] as usize) % 35] as u32) | (buffer0[(buffer1[209] as usize) % 20] as u32)) & 24);
    b = weird_rol8(buffer4[(buffer1[127] as usize) % 21] as u32, (buffer2[(buffer3[68] as usize) % 35] as u32) & 7);
    c = (a & (buffer0[10] as u32)) | (b & !(buffer0[10] as u32));
    d = 7 ^ ((buffer4[(buffer2[(buffer3[36] as usize) % 35] as usize) % 21] as u32) << 1);
    buffer3[72] = ((c & 71) | (d & !71)) as u8;

    buffer2[2] = buffer2[2].wrapping_add(
        ((((((buffer0[(buffer3[20] as usize) % 20] as u32) << 1) & 159)
            | ((buffer4[(buffer1[190] as usize) % 21] as u32) & !159))
            & (((((buffer4[(buffer3[64] as usize) % 21] as u32) & 110)
                | ((buffer0[(buffer1[25] as usize) % 20] as u32) & !110))
                & !150)
                | ((buffer1[25] as u32) & 150)))) as u8,
    );

    buffer2[14] = buffer2[14].wrapping_sub(
        (((buffer2[(buffer3[20] as usize) % 35] as u32) & ((buffer3[72] as u32) ^ (buffer2[(buffer1[100] as usize) % 35] as u32)) & !34)
            | ((buffer1[97] as u32) & 34)) as u8,
    );

    buffer0[17] = 115;

    buffer1[23] ^= (((((((buffer4[(buffer1[17] as usize) % 21] as u32) | (buffer0[(buffer3[20] as usize) % 20] as u32)) & (buffer3[72] as u32))
        | ((buffer4[(buffer1[17] as usize) % 21] as u32) & (buffer0[(buffer3[20] as usize) % 20] as u32))) & ((buffer1[50] as u32) / 3))
        | (((((buffer4[(buffer1[17] as usize) % 21] as u32) | (buffer0[(buffer3[20] as usize) % 20] as u32)) & (buffer3[72] as u32))
            | ((buffer4[(buffer1[17] as usize) % 21] as u32) & buffer0[(buffer3[20] as usize) % 20] as u32)
            | ((buffer1[50] as u32) / 3)) & 246)) as u8)
        .wrapping_shl(1) as u8;

    buffer0[13] = ((((((buffer0[(buffer3[40] as usize) % 20] as u32) | (buffer1[10] as u32)) & 82)
        | ((buffer0[(buffer3[40] as usize) % 20] as u32) & (buffer1[10] as u32))) & 209)
        | (((buffer0[(buffer1[39] as usize) % 20] as u32) << 1) & 46)) as u8
        >> 1;

    buffer2[33] = buffer2[33].wrapping_sub((buffer1[113] as u32 & 9) as u8);

    buffer2[28] = buffer2[28].wrapping_sub(
        ((((2u32 | (buffer1[110] as u32 & 222)) >> 1) & !223) | ((buffer3[20] as u32) & 223)) as u8,
    );

    j = weird_rol8(v | z, u & 7); // OK
    a = ((buffer2[16] as u32) & t) | (w & (!(buffer2[16] as u32)));
    b = ((buffer1[33] as u32) & 17) | (x & !17);
    e = (y | ((a + b) / 5)) & 147 | (y & ((a + b) / 5)); // OK
    m = ((buffer3[40] as u32) & (buffer4[((((buffer3[8] as u32).wrapping_add(j).wrapping_add(e)) & 0xff) as usize) % 21] as u32))
        | (((buffer3[40] as u32) | (buffer4[((((buffer3[8] as u32).wrapping_add(j).wrapping_add(e)) & 0xff) as usize) % 21] as u32)) & (buffer2[23] as u32));

    buffer0[15] = ((((((buffer4[(buffer3[20] as usize) % 21] as u32).wrapping_sub(48)) & (!(buffer1[184] as u32)))
        | ((buffer4[(buffer3[20] as usize) % 21] as u32).wrapping_sub(48) & 189)
        | (189 & !(buffer1[184] as u32))) & (m.wrapping_mul(m).wrapping_mul(m)))) as u8;

    buffer2[22] = buffer2[22].wrapping_add(buffer1[183]);

    buffer3[76] = ((3u32.wrapping_mul(buffer4[(buffer1[1] as usize) % 21] as u32)) ^ (buffer3[0] as u32)) as u8;

    a = buffer2[((((buffer3[8] as u32).wrapping_add(j).wrapping_add(e)) & 0xff) as usize) % 35] as u32;
    f = ((((buffer4[(buffer1[178] as usize) % 21] as u32) & a)
        | (((buffer4[(buffer1[178] as usize) % 21] as u32) | a) & 209))
        .wrapping_mul(buffer0[(buffer1[13] as usize) % 20] as u32))
        .wrapping_mul(buffer4[(buffer1[26] as usize) % 21] as u32 >> 1);
    g = (f.wrapping_add(0x733ffff9)).wrapping_mul(198)
        .wrapping_sub(((f.wrapping_add(0x733ffff9)).wrapping_mul(396).wrapping_add(212)) & 212)
        .wrapping_add(85);
    buffer3[80] = ((buffer3[36] as u32 + (g ^ 148).wrapping_add((g ^ 107) << 1).wrapping_sub(127))) as u8;

    buffer3[84] = ((buffer2[(buffer3[64] as usize) % 35] as u32) & 245 | (buffer2[(buffer3[20] as usize) % 35] as u32) & 10) as u8;

    a = (buffer0[(buffer3[68] as usize) % 20] as u32) | 81;
    buffer2[18] = buffer2[18].wrapping_sub(
        (((a.wrapping_mul(a).wrapping_mul(a)) & !(buffer0[15] as u32)) | (((buffer3[80] as u32) / 15) & (buffer0[15] as u32))) as u8,
    );

    buffer3[88] = ((buffer3[8] as u32).wrapping_add(j).wrapping_add(e)
        .wrapping_sub(buffer0[(buffer1[160] as usize) % 20] as u32)
        .wrapping_add((buffer4[(buffer0[((buffer3[8] as u32).wrapping_add(j).wrapping_add(e) & 255) as usize % 20] as u32 % 21) as usize] as u32 / 3))) as u8;

    b = ((r ^ (buffer3[72] as u32)) & !198) | ((s.wrapping_mul(s)) & 198);
    f = ((buffer4[(buffer1[69] as usize) % 21] as u32) & (buffer1[172] as u32))
        | (((buffer4[(buffer1[69] as usize) % 21] as u32) | (buffer1[172] as u32)) & (((buffer3[12] as u32).wrapping_sub(b)) + 77));
    buffer0[16] = (147u32.wrapping_sub(
        ((buffer3[72] as u32) & ((f & 251) | 1)) | (((f & 250) | (buffer3[72] as u32)) & 198),
    )) as u8;

    c = ((buffer4[(buffer1[168] as usize) % 21] as u32) & buffer0[(buffer1[29] as usize) % 20] as u32 & 7)
        | ((buffer4[(buffer1[168] as usize) % 21] as u32 | buffer0[(buffer1[29] as usize) % 20] as u32) & 6);
    f = ((buffer4[(buffer1[155] as usize) % 21] as u32) & (buffer1[105] as u32))
        | (((buffer4[(buffer1[155] as usize) % 21] as u32) | (buffer1[105] as u32)) & 141);
    buffer0[3] = buffer0[3].wrapping_sub(buffer4[(weird_rol32(f, c) as usize) % 21]);

    // 注意：按位取反后符号扩展为 64 位，再做 64 位除法
    buffer1[5] = (weird_ror8(buffer0[12] as u32, ((buffer0[(buffer1[61] as usize) % 20] as u32) / 5) & 7) as i32 as i64
        ^ ((!buffer2[(buffer3[84] as usize) % 35] as u32) as i32 as i64 / 5)) as u8;

    buffer1[198] = buffer1[198].wrapping_add(buffer1[3]);

    a = 162u32 | (buffer2[(buffer3[64] as usize) % 35] as u32);
    buffer1[164] = buffer1[164].wrapping_add((a.wrapping_mul(a) / 5) as u8);

    g = weird_ror8(139, (buffer3[80] as u32) & 7);
    c = ((buffer4[(buffer3[64] as usize) % 21] as u32)
        .wrapping_mul(buffer4[(buffer3[64] as usize) % 21] as u32)
        .wrapping_mul(buffer4[(buffer3[64] as usize) % 21] as u32)
        & 95)
        | ((buffer0[(buffer3[40] as usize) % 20] as u32) & !95);
    buffer3[92] = ((g & 12) | ((buffer0[(buffer3[20] as usize) % 20] as u32) & 12) | (g & (buffer0[(buffer3[20] as usize) % 20] as u32)) | c) as u8;

    buffer2[12] = buffer2[12].wrapping_add(
        ((((buffer1[103] as u32) & 32) | ((buffer3[92] as u32) & ((buffer1[103] as u32) | 60)) | 16) / 3) as u8,
    );

    buffer3[96] = buffer1[143];

    buffer3[100] = 27;

    buffer3[104] = ((((buffer3[40] as u32) & !(buffer2[8] as u32)) | ((buffer1[35] as u32) & (buffer2[8] as u32))) & (buffer3[64] as u32) ^ 119) as u8;

    buffer3[108] = (238u32 & (((((buffer3[40] as u32) & !(buffer2[8] as u32)) | ((buffer1[35] as u32) & (buffer2[8] as u32))) & (buffer3[64] as u32)) << 1)) as u8;

    buffer3[112] = ((!(buffer3[64] as u32) & ((buffer3[84] as u32) / 3)) ^ 49) as u8;

    buffer3[116] = (98u32 & ((!(buffer3[64] as u32) & ((buffer3[84] as u32) / 3)) << 1)) as u8;

    // finale
    a = ((buffer1[35] as u32) & (buffer2[8] as u32)) | ((buffer3[40] as u32) & !(buffer2[8] as u32));
    b = (a & buffer3[64] as u32) | ((((buffer3[84] as u32) / 3) & !(buffer3[64] as u32)));
    buffer1[143] = ((buffer3[96] as u32)
        .wrapping_sub(
            (b & (86 + (((buffer1[172] as u32) & 64) >> 1)))
                | ((((((buffer1[172] as u32) & 65) >> 1) ^ 86)
                    | ((!(buffer3[64] as u32) & ((buffer3[84] as u32) / 3))
                        | ((((buffer3[40] as u32) & !(buffer2[8] as u32)) | ((buffer1[35] as u32) & (buffer2[8] as u32))) & (buffer3[64] as u32))))
                    & (buffer3[100] as u32)),
        )) as u8;

    buffer2[29] = 162;

    a = ((((buffer4[(buffer3[88] as usize) % 21] as u32) & 160) | (buffer0[(buffer1[125] as usize) % 20] as u32 & 95)) >> 1);
    b = (buffer2[(buffer1[149] as usize) % 35] as u32) ^ ((buffer1[43] as u32).wrapping_mul(buffer1[43] as u32));

    buffer0[15] = buffer0[15].wrapping_add(((b & a) | ((a | b) & 115)) as u8);

    buffer3[120] = ((buffer3[64] as u32).wrapping_sub(buffer0[(buffer3[40] as usize) % 20] as u32)) as u8;

    buffer1[95] = buffer4[(buffer3[20] as usize) % 21];

    a = weird_ror8(
        buffer2[(buffer3[80] as usize) % 35] as u32,
        (buffer2[(buffer1[17] as usize) % 35] as u32)
            .wrapping_mul(buffer2[(buffer1[17] as usize) % 35] as u32)
            .wrapping_mul(buffer2[(buffer1[17] as usize) % 35] as u32)
            & 7,
    );
    buffer0[7] = buffer0[7].wrapping_sub((a.wrapping_mul(a)) as u8);

    buffer2[8] = ((buffer2[8] as u32)
        .wrapping_sub(buffer1[184] as u32)
        .wrapping_add(
            (buffer4[(buffer1[202] as usize) % 21] as u32)
                .wrapping_mul(buffer4[(buffer1[202] as usize) % 21] as u32)
                .wrapping_mul(buffer4[(buffer1[202] as usize) % 21] as u32),
        )) as u8;

    buffer0[16] = ((buffer2[(buffer1[102] as usize) % 35] as u32) << 1 & 132) as u8;

    buffer3[124] = (((buffer4[(buffer3[40] as usize) % 21] as u32) >> 1) ^ (buffer3[68] as u32)) as u8;

    buffer0[7] = buffer0[7].wrapping_sub(
        (buffer0[(buffer1[191] as usize) % 20] as u32)
            .wrapping_sub(
                ((buffer4[(buffer1[80] as usize) % 21] as u32) << 1 & !177)
                    | (buffer4[(buffer4[(buffer3[88] as usize) % 21] as usize) % 21] as u32 & 177),
            ) as u8,
    );

    buffer0[6] = buffer0[(buffer1[119] as usize) % 20];

    a = (buffer4[(buffer1[190] as usize) % 21] as u32 & !209) | (buffer1[118] as u32 & 209);
    b = buffer0[(buffer3[120] as usize) % 20] as u32 * buffer0[(buffer3[120] as usize) % 20] as u32;
    buffer0[12] = ((buffer0[(buffer3[84] as usize) % 20] as u32
        ^ (buffer2[(buffer1[71] as usize) % 35] as u32 + buffer2[(buffer1[15] as usize) % 35] as u32))
        & ((a & b) | ((a | b) & 27))) as u8;

    b = ((buffer1[32] as u32) & (buffer2[(buffer3[88] as usize) % 35] as u32))
        | (((buffer1[32] as u32) | (buffer2[(buffer3[88] as usize) % 35] as u32)) & 23);
    d = (((buffer4[(buffer1[57] as usize) % 21] as u32) * 231) & 169) | (b & 86);
    f = ((((buffer0[(buffer1[82] as usize) % 20] as u32) & !29) | ((buffer4[(buffer3[124] as usize) % 21] as u32) & 29)) & 190)
        | (buffer4[((d / 5) as usize) % 21] as u32 & !190);
    h = buffer0[(buffer3[40] as usize) % 20] as u32
        * buffer0[(buffer3[40] as usize) % 20] as u32
        * buffer0[(buffer3[40] as usize) % 20] as u32;
    k = (h & (buffer1[82] as u32)) | (h & 92) | ((buffer1[82] as u32) & 92);
    buffer3[128] = (((f & k) | ((f | k) & 192)) ^ (d / 5)) as u8;

    buffer2[25] ^= (((buffer0[(buffer3[120] as usize) % 20] as u32) << 1).wrapping_mul(buffer1[5] as u32))
        .wrapping_sub(weird_rol8(buffer3[76] as u32, buffer4[(buffer3[124] as usize) % 21] as u32 & 7) & (buffer3[20] as u32 + 110)) as u8;
}
