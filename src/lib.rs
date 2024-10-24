// #![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unreachable_code)]
#![allow(non_snake_case)]
#![allow(clippy::clone_on_copy)]
#![allow(unused_mut)]

#[cfg(test)] mod tests;

pub fn ghash(hashkey: [u8; 16], blocks: &[[u8; 16]]) -> [u8; 16] {
    let mut x = [0u8; 16];

    for block in blocks {
        for i in 0..16 {
            x[i] ^= block[i];
        }
        x = gfmul(x, hashkey);
    }

    x
}

pub fn gfmul(a: [u8; 16], b: [u8; 16]) -> [u8; 16] {
    let a_uint = parse_array_as_uint(a);
    let b_arr = parse_array_as_bits(b);

    // obtain a 128 bit array of each element in b_arr times a_uint
    //
    let products: Vec<u128> =
        b_arr.iter().enumerate().map(|(i, bit)| if *bit { a_uint << i } else { 0 }).collect();

    // accumulate the xor of each 128-bit element into the pieces of upper_128 and lower_128
    // a * b_arr[0]   => bits 0..128
    // a * b_arr[1]   => bits 1..129
    // ...
    // a * b_arr[127] => bits 127..255
    //
    // where:
    // lower128 contains bits 0..128
    // upper128 contains bits 128..256
    let (mut upper128, mut lower128) = (0, 0);
    for product in products {
        lower128 ^= product & ((1 << 128) - 1);
        upper128 ^= product >> 128;
    }

    todo!()
}

/// Multiplication over the finite field $\text{GF}(2^{128})$. Elements in this field are 128-bit
/// binary vectors, and arithmetic operations are defined modulo the irreducible polynomial:
/// $x^{128} + x^7 + x^2 + x + 1$.
///
/// sadmode_gfmul sadly makes incorrect assumptions about the feasibility of performing galois field
/// arithmetic within the integers, and is irretrievably incorrect.
pub fn _sadmode_gfmul(a: [u8; 16], b: [u8; 16]) -> [u8; 16] {
    let (al, ar) = parse_array_as_pair(a);
    let (bl, br) = parse_array_as_pair(b);
    // println!("al: {:?}, ar: {:?}, bl: {:?}, br: {:?}", al, ar, bl, br);

    // bits 0..128
    let rr = ar * br;
    // bits 64..192
    let lr = (al * br) ^ (ar * bl);
    // bits 128..256
    let ll = al * bl;
    // println!("ll: {:?}, rr: {:?}, _lr: {:?}, _rl: {:?}, lr: {lr:?}", ll, rr, al * br, ar * bl);

    // sieve to upper 128..256 bits and lower 128 bits
    let (lr_hi, lr_lo) = (lr >> 64, (lr & (2u128.pow(64) - 1)));

    // println!("lr_hi: {:?}, lr_lo: {:?}", lr_hi, lr_lo);
    let (upper, lower) = (ll ^ lr_hi, rr ^ (lr_lo << 64));
    // println!("upper: {:?}, lower: {:?}", upper, lower);

    // reduce the upper 128 bits back into the field
    // println!("galois_reduce(upper): {:?}", galois_reduce(upper));
    parse_u128_as_array(lower ^ galois_reduce(upper))
}

/// Compute x^{128} * POLY_n (mod x^{128} + 1 + x + x^2 + x^7)
///
/// where POLY_n encodes a Galois polynomial according to the GHash convention.
///
/// e.g.
/// n=1   : x^128 * x^0 = 1   + x   + x^2 + x^7 ; return 135
/// n=2   : x^128 * x^1 = x   + x^2 + x^3 + x^8 ; return 270
/// n=3   : f(1) ^ f(2)                         ; return 270 ^ 135
/// n=4   : x^128 * x^2 = x^2 + x^3 + x^4 + x^9 ; return 540
/// 1<<120: x^128*x^120 = x^120+x^121+x^122+x^127
/// 1<<121: x^128*x^121 = x^121+x^122+x^123+(x^0+x^1+x^2+x^7)
fn galois_reduce(n: u128) -> u128 {
    let mut m = 0u128;

    // 126: since the product x^127 * x^127 is at most 254
    // therefore there is no element of degree greater than 126=254-126
    (0..=127).for_each(|i| {
        if n & (1 << i) != 0 {
            // println!("{n} ^ (1 << {i}) = {}", n ^ (1 << i));
            // println!("gal_product_int({i})={}", galois_product_int(i));
            m ^= galois_product_int(i);
        }
    });

    // let reduced: i32 = (1..10).reduce(|acc, e| acc + e).unwrap();
    let m = (0..=127)
        .map(|i| if n & (1 << i) != 0 { galois_product_int(i) } else { 0 })
        // .reduce(|acc, e| acc ^ e)
    // .unwrap();
        .fold(0, |acc, e| acc ^ e);
    // println!("galois_reduced: {m}");

    // pythonic:

    m
}

/// Computes galois polynomial product (x^n)(x^7 + x^2 + x + 1) encoded as u128
///
/// n=0  : [1, 1, 1, 0, 0, 0, 0, 1, 0...] => 135
/// n=1  : [0, 1, 1, 1, 0, 0, 0, 0, 1, 0...] => 270
/// n=121: (121, 122, 123, (128=>0,1,2,7)) sum of 2 to each of these values
fn galois_product_int(n: u8) -> u128 {
    galois_product(n).into_iter().rev().fold(0, |acc, i| (acc << 1) | (i as u128))
}

/// Computes galois polynomial product (x^n)(x^7 + x^2 + x + 1)
/// returns an array with leading LSB.
///
/// e.g.
/// n=0: [1, 1, 1, 0, 0, 0, 0, 1, 0...]
/// n=1: [0, 1, 1, 1, 0, 0, 0, 0, 1, 0...]
fn galois_product(n: u8) -> [u8; 128] {
    assert!(n < 128);
    let mut v = [0; 128];

    for j in [0, 1, 2, 7] {
        if n + j < 128 {
            v[(n + j) as usize] ^= 1;
        } else {
            for k in [0, 1, 2, 7] {
                v[(n + j + k - 128) as usize] ^= 1;
            }
        }
    }
    v
}

/// Note that these bytes are neither BE nor LE encoded.
/// Leading bit is LSB; trailing bit is MSB.
///
/// Thus:
///     1 = [ 1, 0, 0, 0, ... 0 ]
/// 2^127 = [ 0, 0, 0, ... 0, 1 ]
///
/// if byte b = [1 0 0 0 0 0 0 0]
///
/// Return: (MSB right parsed 64-bits, LSB left parsed 64 bits))
fn parse_array_as_pair(arr: [u8; 16]) -> (u128, u128) {
    // ghash uses reversed internal byte-order
    let arr = (arr).into_iter().map(reverse_byte).collect::<Vec<u8>>();
    let (lower, upper) = arr.split_at(8);
    let lower = (0..8).fold(0, |acc, i| acc | (lower[i] as u128) << (i * 8));
    let upper = (0..8).fold(0, |acc, i| acc | (upper[i] as u128) << (i * 8));

    (upper, lower)
}

/// parse ghash-convention byte array to uint
fn parse_array_as_uint(arr: [u8; 16]) -> u128 {
    // ghash uses reversed internal byte-order
    let arr = (arr).into_iter().map(reverse_byte).collect::<Vec<u8>>();
    (0..16).fold(0, |acc, i| acc | (arr[i] as u128) << (i * 8))
}

/// parse ghash-convention byte array to ghash-convention bits
fn parse_array_as_bits(arr: [u8; 16]) -> [bool; 128] {
    (0..16).fold([false; 128], |mut acc, i| {
        let bits = parse_u8_as_bits(reverse_byte(arr[i]));
        (0..8).for_each(|j| acc[i * 8 + j] = bits[j]);
        acc
    })
}

/// interpret 128; i.e. 0x80 as [1000 0000]
fn parse_u8_as_bits(b: u8) -> [bool; 8] { core::array::from_fn(|i| (b & (1 << i)) != 0) }

/// send bits in byte to reverse order; e.g. send (192=128+64) -> 3
fn reverse_byte(b: u8) -> u8 { (0..8).fold(0, |acc, i| acc | ((b >> (7 - i)) & 1) << i) }

/// parse u128 into ghash custom reversed-byte array
/// e.g.
/// 1 << 127 -> [ 0x80 0x00...]
/// 1        -> [ 0x00... 0x01]
fn parse_u128_as_array(n: u128) -> [u8; 16] {
    let mut arr = [0; 16];
    for i in 0..16 {
        arr[i] = reverse_byte((n >> (i * 8)) as u8);
        // println!("{i}th byte of {n}: {}", arr[i]);
    }
    arr
}
