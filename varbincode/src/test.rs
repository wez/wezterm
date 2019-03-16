use super::{deserialize, serialize};
use serde::Serialize;
use serde_derive::*;
use std::collections::HashMap;

fn same<'de, T: serde::de::DeserializeOwned + Serialize + std::fmt::Debug + PartialEq>(a: T) {
    let encoded = serialize(&a).unwrap();
    let decoded: T = deserialize(encoded.as_slice()).unwrap();
    assert_eq!(decoded, a);
    eprintln!("{:?} encoded as {:?}", a, encoded);
}

#[test]
fn test() {
    same(0u8);
    same(1u8);
    same(1i8);
    same(0i8);
    same(0u16);
    same(255u16);
    same(0xffffu16);
    same(0x7fffi16);
    same(-0x7fffi16);
    same(0x00ff_ffffu32);
    same(0xffff_ffffu32);
    same(0x00ff_ffffu64);
    same(0xffff_ffffu64);
    same(0xffff_ffff_ffffu64);
    same(0xffff_ffff_ffff_ffffu64);
    same(0f32);
    same(10.5f32);
    same(10.5f64);
    same(-10.5f64);

    same("".to_string());
    same("hello".to_string());

    same((1u8,));
    same((1u8, 2, 3));
    same((1u8, "foo".to_string()));

    same(true);
    same(false);

    same(Some(true));
    same(None::<bool>);

    same('c');
    same(b'c');
}

#[test]
fn test_structs() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Struct {
        a: isize,
        b: String,
        c: bool,
    };

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct Outer {
        inner: Struct,
        b: bool,
        second: Struct,
    };

    same(Struct {
        a: -42,
        b: "hello".to_string(),
        c: true,
    });

    same(Outer {
        inner: Struct {
            a: 1,
            b: "bee".to_string(),
            c: false,
        },
        b: true,
        second: Struct {
            a: 2,
            b: "other".to_string(),
            c: true,
        },
    });

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct NewType(usize);
    same(NewType(123));

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct NewTypeTuple(usize, bool);
    same(NewTypeTuple(123, true));
}

#[test]
fn test_enum() {
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum TestEnum {
        NoArg,
        OneArg(usize),
        Args(usize, usize),
        AnotherNoArg,
        StructLike { x: usize, y: f32 },
    }
    same(TestEnum::NoArg);
    same(TestEnum::OneArg(4));
    same(TestEnum::Args(4, 5));
    same(TestEnum::AnotherNoArg);
    same(TestEnum::StructLike { x: 4, y: 3.14159 });
    same(vec![
        TestEnum::NoArg,
        TestEnum::OneArg(5),
        TestEnum::AnotherNoArg,
        TestEnum::StructLike { x: 4, y: 1.4 },
    ]);
}

#[test]
fn test_vec() {
    let v: Vec<u8> = vec![];
    same(v);
    same(vec![1u64]);
    same(vec![1u64, 2, 3, 4, 5, 6]);
}

#[test]
fn test_map() {
    let mut m = HashMap::new();
    m.insert(4u64, "foo".to_string());
    m.insert(0u64, "bar".to_string());
    same(m);
}

#[test]
fn test_fixed_size_array() {
    same([24u32; 32]);
    same([1u64, 2, 3, 4, 5, 6, 7, 8]);
    same([0u8; 19]);
}
