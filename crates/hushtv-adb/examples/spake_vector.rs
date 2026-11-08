//! テスト用：決まった乱数で SPAKE2 の値を出す（BoringSSL の手順を写した Python と突き合わせる）。
use hushtv_adb::pair::spake2::{Role, Spake2};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    for seed in [1u8, 77, 200] {
        let ra: [u8; 64] = std::array::from_fn(|i| seed.wrapping_add(i as u8).wrapping_mul(31));
        let rb: [u8; 64] =
            std::array::from_fn(|i| seed.wrapping_mul(3).wrapping_add(i as u8).wrapping_mul(17));
        let pw = format!("{:06}", seed as u32 * 4111);
        let a = Spake2::new(
            Role::Alice,
            b"adb pair client\0",
            b"adb pair server\0",
            pw.as_bytes(),
            &ra,
        );
        let b = Spake2::new(
            Role::Bob,
            b"adb pair server\0",
            b"adb pair client\0",
            pw.as_bytes(),
            &rb,
        );
        let (am, bm) = (*a.message(), *b.message());
        let k = a.finish(&bm).unwrap();
        println!("{} {} {} {}", seed, hex(&am), hex(&bm), hex(&k));
    }
}
