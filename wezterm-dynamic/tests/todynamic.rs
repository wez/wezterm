use maplit::btreemap;
use ordered_float::OrderedFloat;
use wezterm_dynamic::{ToDynamic, Value};

#[test]
fn intrinsics() {
    assert_eq!(23u8.to_dynamic(), Value::U64(23));
    assert_eq!(23i8.to_dynamic(), Value::I64(23));
    assert_eq!(23f32.to_dynamic(), Value::F64(OrderedFloat(23.)));
    assert_eq!("hello".to_dynamic(), Value::String("hello".to_string()));
    assert_eq!(false.to_dynamic(), Value::Bool(false));
}

#[derive(ToDynamic, Debug, PartialEq)]
struct SimpleStruct {
    age: u8,
}

#[test]
fn simple_struct() {
    assert_eq!(
        SimpleStruct { age: 42 }.to_dynamic(),
        Value::Object(
            btreemap!(
                "age".to_dynamic() => Value::U64(42))
            .into()
        )
    );
}

#[derive(ToDynamic, Debug, PartialEq)]
struct SimpleStructWithRenamedField {
    #[dynamic(rename = "how_old")]
    age: u8,
}

#[test]
fn simple_struct_with_renamed_field() {
    assert_eq!(
        SimpleStructWithRenamedField { age: 42 }.to_dynamic(),
        Value::Object(
            btreemap!(
                "how_old".to_dynamic() => Value::U64(42))
            .into()
        )
    );
}

#[derive(ToDynamic, Debug, PartialEq)]
struct StructWithSkippedField {
    #[dynamic(skip)]
    admin: bool,
    age: u8,
}

#[test]
fn skipped_field() {
    assert_eq!(
        StructWithSkippedField {
            admin: true,
            age: 42
        }
        .to_dynamic(),
        Value::Object(
            btreemap!(
                "age".to_dynamic() => Value::U64(42))
            .into()
        )
    );
}

#[derive(ToDynamic, Debug, PartialEq)]
struct StructWithFlattenedStruct {
    top: bool,
    #[dynamic(flatten)]
    simple: SimpleStruct,
}

#[test]
fn flattened() {
    assert_eq!(
        StructWithFlattenedStruct {
            top: true,
            simple: SimpleStruct { age: 42 }
        }
        .to_dynamic(),
        Value::Object(
            btreemap!(
                "top".to_dynamic() => Value::Bool(true),
                "age".to_dynamic() => Value::U64(42))
            .into()
        )
    );
}

#[derive(ToDynamic, Debug, PartialEq)]
enum Units {
    A,
    B,
}

#[test]
fn unit_variants() {
    assert_eq!(Units::A.to_dynamic(), Value::String("A".to_string()));
    assert_eq!(Units::B.to_dynamic(), Value::String("B".to_string()));
}

#[derive(ToDynamic, Debug, PartialEq)]
enum Named {
    A { foo: bool, bar: bool },
    B { bar: bool },
}

#[test]
fn named_variants() {
    assert_eq!(
        Named::A {
            foo: true,
            bar: false
        }
        .to_dynamic(),
        Value::Object(
            btreemap!(
                "A".to_dynamic() => Value::Object(
                    btreemap!(
                        "foo".to_dynamic() => Value::Bool(true),
                        "bar".to_dynamic() => Value::Bool(false),
                    ).into())
            )
            .into()
        )
    );
    assert_eq!(
        Named::B { bar: true }.to_dynamic(),
        Value::Object(
            btreemap!(
                "B".to_dynamic() => Value::Object(
                    btreemap!(
                        "bar".to_dynamic() => Value::Bool(true),
                    ).into())
            )
            .into()
        )
    );
}

#[derive(ToDynamic, Debug, PartialEq)]
enum UnNamed {
    A(f32, f32, f32, f32),
    Single(bool),
}

#[test]
fn unnamed_variants() {
    assert_eq!(
        UnNamed::A(0., 1., 2., 3.).to_dynamic(),
        Value::Object(
            btreemap!(
                "A".to_dynamic() => Value::Array(vec![
                    Value::F64(OrderedFloat(0.)),
                    Value::F64(OrderedFloat(1.)),
                    Value::F64(OrderedFloat(2.)),
                    Value::F64(OrderedFloat(3.)),
                ].into()),
            )
            .into()
        )
    );

    assert_eq!(
        UnNamed::Single(true).to_dynamic(),
        Value::Object(
            btreemap!(
                "Single".to_dynamic() => Value::Bool(true),
            )
            .into()
        )
    );
}

#[derive(ToDynamic, Debug, PartialEq)]
#[dynamic(into = "String")]
struct StructInto {
    age: u8,
}

impl Into<String> for &StructInto {
    fn into(self) -> String {
        format!("age:{}", self.age)
    }
}

#[test]
fn struct_into() {
    assert_eq!(
        StructInto { age: 42 }.to_dynamic(),
        Value::String("age:42".to_string())
    );
}

#[derive(ToDynamic, Debug, PartialEq)]
#[dynamic(into = "String")]
enum EnumInto {
    Age(u8),
}

impl Into<String> for &EnumInto {
    fn into(self) -> String {
        match self {
            EnumInto::Age(age) => format!("age:{}", age),
        }
    }
}

#[test]
fn enum_into() {
    assert_eq!(
        EnumInto::Age(42).to_dynamic(),
        Value::String("age:42".to_string())
    );
}
