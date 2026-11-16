use std::sync::OnceLock;

/// 256x256 character-pair frequency weight table (128 KB of u16 weights)
/// In Cursor's crepe engine, weights inversely correlate with frequency in 2TB of code:
/// Rare pairs -> High weight (up to 65535)
/// Frequent pairs -> Low weight (down to 10)
pub static FREQUENCY_TABLE: OnceLock<Box<[u16; 65536]>> = OnceLock::new();

pub fn get_pair_weight(c1: u8, c2: u8) -> u16 {
    let table = FREQUENCY_TABLE.get_or_init(init_frequency_table);
    let idx = ((c1 as usize) << 8) | (c2 as usize);
    table[idx]
}

fn init_frequency_table() -> Box<[u16; 65536]> {
    let mut table = Box::new([0u16; 65536]);

    // Baseline deterministic pseudo-random distribution via murmur/splitmix
    for c1 in 0..=255u8 {
        for c2 in 0..=255u8 {
            let idx = ((c1 as usize) << 8) | (c2 as usize);
            let mut h = (c1 as u64)
                .wrapping_mul(0x517cc1b727220a95)
                .wrapping_add((c2 as u64).wrapping_mul(0x4f1bbcdcbfa54001));
            h = (h ^ (h >> 32)).wrapping_mul(0x9e3779b97f4a7c15);
            h = (h ^ (h >> 32)).wrapping_mul(0xc6a4a7935bd1e995);
            let raw_weight = ((h ^ (h >> 32)) % 40000) as u16 + 10000;
            table[idx] = raw_weight;
        }
    }

    // Calibration: Ultra-frequent whitespace and common English digraphs get very low weights (10 - 2000)
    let common_pairs = [
        (b' ', b' '), (b' ', b'\t'), (b'\t', b' '), (b'\t', b'\t'),
        (b'\r', b'\n'), (b'\n', b' '), (b'\n', b'\t'),
        (b't', b'h'), (b'h', b'e'), (b'i', b'n'), (b'e', b'r'), (b'a', b'n'),
        (b'r', b'e'), (b'o', b'n'), (b'a', b't'), (b'e', b'n'), (b'n', b'd'),
        (b't', b'i'), (b'e', b's'), (b'o', b'r'), (b't', b'e'), (b'o', b'f'),
        (b'e', b'd'), (b'i', b's'), (b'i', b't'), (b'a', b'l'), (b'a', b'r'),
        (b's', b't'), (b't', b'o'), (b'n', b't'), (b's', b'e'), (b'h', b'a'),
        (b'a', b's'), (b'o', b'u'), (b'n', b'g'), (b'c', b'o'), (b'd', b'e'),
        (b'l', b'e'), (b'm', b'e'), (b'f', b'o'), (b'r', b' '), (b'i', b'f'),
    ];

    for &(c1, c2) in &common_pairs {
        let idx = ((c1 as usize) << 8) | (c2 as usize);
        table[idx] = 100;
    }

    // Code keywords & syntactical pairs get high weights (50000 - 65535)
    let rare_syntax_pairs = [
        (b':', b':'), // :: namespace
        (b'-', b'>'), // -> arrow
        (b'=', b'>'), // => fat arrow
        (b'!', b'='), // != not equal
        (b'=', b'='), // == equality
        (b'&', b'&'), // && logical and
        (b'|', b'|'), // || logical or
        (b'#', b'['), // #[ attribute
        (b'$', b'{'), // ${ template
        (b'}', b'}'), // }} template close
        (b'<', b'>'), // <> generics
        (b'/', b'*'), // /* comment start
        (b'*', b'/'), // */ comment end
        (b';', b';'),
        (b'~', b'='),
        (b'^', b'='),
        (b'%', b'='),
        (b'?', b'.'), // ?. optional chaining
        (b'?', b'?'), // ?? nullish coalescing
        (b'.', b'.'), // .. range
    ];

    for &(c1, c2) in &rare_syntax_pairs {
        let idx = ((c1 as usize) << 8) | (c2 as usize);
        table[idx] = 65000;
    }

    // Lowercase to Uppercase transitions (camelCase / PascalCase boundaries) get very high weights
    for lower in b'a'..=b'z' {
        for upper in b'A'..=b'Z' {
            let idx = ((lower as usize) << 8) | (upper as usize);
            table[idx] = 58000;
        }
    }

    // Underscore to alphanumeric transitions (snake_case boundaries)
    for c in (b'a'..=b'z').chain(b'0'..=b'9') {
        let idx1 = ((b'_' as usize) << 8) | (c as usize);
        let idx2 = ((c as usize) << 8) | (b'_' as usize);
        table[idx1] = 55000;
        table[idx2] = 55000;
    }

    table
}
