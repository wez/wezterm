use maplit::btreemap;
use ordered_float::OrderedFloat;
use wezterm_dynamic::{Error, FromDynamic, Object, ToDynamic, Value};

#[derive(FromDynamic, Debug, PartialEq)]
struct SimpleStruct {
    age: u8,
}

#[test]
fn simple_struct() {
    let s = SimpleStruct::from_dynamic(
        &Value::Object(
            btreemap!(
            "age".to_dynamic() => Value::U64(42))
            .into(),
        ),
        Default::default(),
    )
    .unwrap();
    assert_eq!(s, SimpleStruct { age: 42 });
}

#[derive(FromDynamic, Debug, PartialEq, Default)]
struct StructWithSkippedField {
    #[dynamic(skip)]
    admin: bool,
    age: u8,
}

#[test]
fn skipped_field() {
    let s = StructWithSkippedField::from_dynamic(
        &Value::Object(
            btreemap!(
            "age".to_dynamic() => Value::U64(42))
            .into(),
        ),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        s,
        StructWithSkippedField {
            age: 42,
            admin: false
        }
    );
}

#[derive(FromDynamic, Debug, PartialEq)]
struct StructWithFlattenedStruct {
    top: bool,
    #[dynamic(flatten)]
    simple: SimpleStruct,
}

#[test]
fn flattened() {
    let s = StructWithFlattenedStruct::from_dynamic(
        &Value::Object(
            btreemap!(
                "top".to_dynamic() =>Value::Bool(true),
                "age".to_dynamic() => Value::U64(42))
            .into(),
        ),
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        s,
        StructWithFlattenedStruct {
            top: true,
            simple: SimpleStruct { age: 42 },
        }
    );
}

#[derive(FromDynamic, Debug, PartialEq)]
enum Units {
    A,
}

#[test]
fn unit_variants() {
    assert_eq!(
        Units::A,
        Units::from_dynamic(&Value::String("A".to_string()), Default::default()).unwrap()
    );
}

#[derive(FromDynamic, Debug, PartialEq)]
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
        },
        Named::from_dynamic(
            &Value::Object(
                btreemap!(
                    "A".to_dynamic() => Value::Object(
                        btreemap!(
                            "foo".to_dynamic() => Value::Bool(true),
                            "bar".to_dynamic() => Value::Bool(false),
                        ).into())
                )
                .into()
            ),
            Default::default()
        )
        .unwrap()
    );
    assert_eq!(
        Named::B { bar: true },
        Named::from_dynamic(
            &Value::Object(
                btreemap!(
                    "B".to_dynamic() => Value::Object(
                        btreemap!(
                            "bar".to_dynamic() => Value::Bool(true),
                        ).into())
                )
                .into()
            ),
            Default::default()
        )
        .unwrap()
    );
}

#[derive(FromDynamic, Debug, PartialEq)]
enum UnNamed {
    A(f32, f32, f32, f32),
    Single(bool),
}

#[test]
fn unnamed_variants() {
    assert_eq!(
        UnNamed::A(0., 1., 2., 3.),
        UnNamed::from_dynamic(
            &Value::Object(
                btreemap!(
                    "A".to_dynamic() => Value::Array(vec![
                        Value::F64(OrderedFloat(0.)),
                        Value::F64(OrderedFloat(1.)),
                        Value::F64(OrderedFloat(2.)),
                        Value::F64(OrderedFloat(3.)),
                    ].into()),
                )
                .into()
            ),
            Default::default()
        )
        .unwrap()
    );

    assert_eq!(
        UnNamed::Single(true),
        UnNamed::from_dynamic(
            &Value::Object(
                btreemap!(
                    "Single".to_dynamic() => Value::Bool(true),
                )
                .into()
            ),
            Default::default()
        )
        .unwrap()
    );
}

#[derive(FromDynamic, Debug, PartialEq)]
struct OptField {
    foo: Option<bool>,
}

#[test]
fn optional() {
    assert_eq!(
        OptField { foo: None },
        OptField::from_dynamic(&Value::Object(Object::default()), Default::default()).unwrap(),
    );

    assert_eq!(
        OptField { foo: Some(true) },
        OptField::from_dynamic(
            &Value::Object(
                btreemap! {
                    "foo".to_dynamic() => Value::Bool(true),
                }
                .into()
            ),
            Default::default()
        )
        .unwrap(),
    );
}

#[derive(FromDynamic, Debug, PartialEq)]
struct Defaults {
    #[dynamic(default)]
    s: String,
    #[dynamic(default = "woot_string")]
    w: String,
}

fn woot_string() -> String {
    "woot".to_string()
}

#[test]
fn defaults() {
    assert_eq!(
        Defaults {
            s: "".to_string(),
            w: "woot".to_string()
        },
        Defaults::from_dynamic(&Value::Object(Object::default()), Default::default()).unwrap(),
    );
}

#[derive(FromDynamic, Debug, PartialEq)]
#[dynamic(try_from = "String")]
struct StructInto {
    age: u8,
}

impl TryFrom<String> for StructInto {
    type Error = String;

    fn try_from(s: String) -> Result<StructInto, Self::Error> {
        if let [label, value] = &s.split(':').collect::<Vec<_>>()[..] {
            if *label == "age" {
                return Ok(StructInto {
                    age: value
                        .parse()
                        .map_err(|e: std::num::ParseIntError| e.to_string())?,
                });
            }
        }
        Err("bad".to_string())
    }
}

#[test]
fn struct_into() {
    assert_eq!(
        StructInto { age: 42 },
        StructInto::from_dynamic(&Value::String("age:42".to_string()), Default::default()).unwrap()
    );
}

#[derive(FromDynamic, Debug, PartialEq)]
#[dynamic(try_from = "String")]
enum EnumInto {
    Age(u8),
}

impl TryFrom<String> for EnumInto {
    type Error = String;

    fn try_from(s: String) -> Result<EnumInto, Self::Error> {
        if let [label, value] = &s.split(':').collect::<Vec<_>>()[..] {
            if *label == "age" {
                return Ok(EnumInto::Age(
                    value
                        .parse()
                        .map_err(|e: std::num::ParseIntError| e.to_string())?,
                ));
            }
        }
        Err("bad".to_string())
    }
}

#[test]
fn enum_into() {
    assert_eq!(
        EnumInto::Age(42),
        EnumInto::from_dynamic(&Value::String("age:42".to_string()), Default::default()).unwrap()
    );
}

// Struct: primary is a named-field struct; simpler format is a plain string.
#[derive(FromDynamic, ToDynamic, Debug, PartialEq)]
#[dynamic(fallback = "String")]
struct StructWithSimplerFallback {
    field: String,
    other: u32,
}
impl From<String> for StructWithSimplerFallback {
    fn from(s: String) -> Self {
        StructWithSimplerFallback { field: s, other: 0 }
    }
}

#[test]
fn fallback_struct_primary_succeeds() {
    // When input matches primary struct format, it is used directly.
    let val = Value::Object(
        btreemap! {
            "field".to_dynamic() => Value::String("hello".to_string()),
            "other".to_dynamic() => Value::U64(99),
        }
        .into(),
    );
    assert_eq!(
        StructWithSimplerFallback {
            field: "hello".to_string(),
            other: 99
        },
        StructWithSimplerFallback::from_dynamic(&val, Default::default()).unwrap(),
    );
}

#[test]
fn fallback_struct_simpler_string_used() {
    // When input is a bare string (simpler format), fallback path runs and From<String>
    // converts it into the primary type.
    let val = Value::String("simpler-value".to_string());
    assert_eq!(
        StructWithSimplerFallback {
            field: "simpler-value".to_string(),
            other: 0
        },
        StructWithSimplerFallback::from_dynamic(&val, Default::default()).unwrap(),
    );
}

#[test]
fn fallback_struct_both_fail_returns_fallback_failed() {
    // When neither primary nor fallback types can deserialize the value,
    // both errors are returned.
    let val = Value::U64(999); // not an Object and not a String
    let err = StructWithSimplerFallback::from_dynamic(&val, Default::default()).unwrap_err();
    let Error::FallbackFailed {
        primary_error,
        fallback_error,
    } = err
    else {
        panic!("expected FallbackFailed, got: {err:?}");
    };
    assert!(
        primary_error
            .to_string()
            .contains("StructWithSimplerFallback"),
        "primary error should mention the primary type name, got: {primary_error}",
    );
    assert!(
        fallback_error.to_string().contains("String"),
        "fallback error should mention the fallback type name, got: {fallback_error}",
    );
}

// Enum: primary is a named-variant enum; simpler format is a plain string.
#[derive(FromDynamic, ToDynamic, Debug, PartialEq)]
#[dynamic(fallback = "String")]
enum EnumWithSimplerFallback {
    Complex { field: String },
    Other,
}

impl From<String> for EnumWithSimplerFallback {
    fn from(s: String) -> Self {
        EnumWithSimplerFallback::Complex { field: s }
    }
}

#[test]
fn fallback_enum_primary_succeeds() {
    // When input matches the primary enum format, it is used directly.
    let val = Value::Object(
        btreemap! {
            "Complex".to_dynamic() => Value::Object(
                btreemap! {
                    "field".to_dynamic() => Value::String("yes".to_string()),
                }
                .into(),
            ),
        }
        .into(),
    );
    assert_eq!(
        EnumWithSimplerFallback::Complex {
            field: "yes".to_string()
        },
        EnumWithSimplerFallback::from_dynamic(&val, Default::default()).unwrap(),
    );
}

#[test]
fn fallback_enum_simpler_string_used() {
    // When input is a bare string, fallback path runs.
    let val = Value::String("simpler-enum".to_string());
    assert_eq!(
        EnumWithSimplerFallback::Complex {
            field: "simpler-enum".to_string()
        },
        EnumWithSimplerFallback::from_dynamic(&val, Default::default()).unwrap(),
    );
}

#[test]
fn fallback_enum_both_fail_returns_fallback_failed() {
    // When neither primary nor fallback types can deserialize the value,
    // both errors are returned.
    let val = Value::U64(42); // not a valid enum Object, not a String
    let err = EnumWithSimplerFallback::from_dynamic(&val, Default::default()).unwrap_err();
    let Error::FallbackFailed {
        primary_error,
        fallback_error,
    } = err
    else {
        panic!("expected FallbackFailed, got: {err:?}");
    };
    assert!(
        primary_error
            .to_string()
            .contains("EnumWithSimplerFallback"),
        "primary error should mention the primary type name, got: {primary_error}",
    );
    assert!(
        fallback_error.to_string().contains("String"),
        "fallback error should mention the fallback type name, got: {fallback_error}",
    );
}
