/// Tests for From/TryFrom conversions and LispVal helpers (issue #56).
use lamedh::{LispError, LispVal};

// ---------------------------------------------------------------------------
// From<T> for LispVal
// ---------------------------------------------------------------------------

#[test]
fn test_from_i64() {
    assert_eq!(LispVal::from(42i64), LispVal::Number(42));
    assert_eq!(LispVal::from(-1i64), LispVal::Number(-1));
}

#[test]
fn test_from_f64() {
    assert_eq!(LispVal::from(3.14f64), LispVal::Float(3.14));
}

#[test]
fn test_from_bool_true() {
    let v = LispVal::from(true);
    // T is represented as a symbol named "T"
    match &v {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        other => panic!("expected Symbol(T), got {other:?}"),
    }
}

#[test]
fn test_from_bool_false() {
    assert_eq!(LispVal::from(false), LispVal::Nil);
}

#[test]
fn test_from_string() {
    assert_eq!(
        LispVal::from("hello".to_string()),
        LispVal::String("hello".to_string())
    );
}

#[test]
fn test_from_str_ref() {
    assert_eq!(LispVal::from("world"), LispVal::String("world".to_string()));
}

#[test]
fn test_from_vec_empty() {
    let v: Vec<LispVal> = vec![];
    assert_eq!(LispVal::from(v), LispVal::Nil);
}

#[test]
fn test_from_vec_one() {
    let v = vec![LispVal::Number(1)];
    let list = LispVal::from(v);
    assert_eq!(
        list,
        LispVal::Cons {
            car: Box::new(LispVal::Number(1)),
            cdr: Box::new(LispVal::Nil),
        }
    );
}

#[test]
fn test_from_vec_multiple() {
    let v = vec![LispVal::Number(1), LispVal::Number(2), LispVal::Number(3)];
    let list = LispVal::from(v);
    // Should produce (1 2 3) as a proper list
    match &list {
        LispVal::Cons { car, cdr } => {
            assert_eq!(**car, LispVal::Number(1));
            match cdr.as_ref() {
                LispVal::Cons { car: car2, cdr: cdr2 } => {
                    assert_eq!(**car2, LispVal::Number(2));
                    match cdr2.as_ref() {
                        LispVal::Cons { car: car3, cdr: tail } => {
                            assert_eq!(**car3, LispVal::Number(3));
                            assert_eq!(**tail, LispVal::Nil);
                        }
                        other => panic!("unexpected tail: {other:?}"),
                    }
                }
                other => panic!("unexpected cdr: {other:?}"),
            }
        }
        other => panic!("expected Cons, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// TryFrom<LispVal> for Rust primitives
// ---------------------------------------------------------------------------

#[test]
fn test_try_from_i64_ok() {
    let v = LispVal::Number(99);
    assert_eq!(i64::try_from(v), Ok(99i64));
}

#[test]
fn test_try_from_i64_err() {
    let v = LispVal::String("nope".to_string());
    assert!(i64::try_from(v).is_err());
}

#[test]
fn test_try_from_f64_from_float() {
    let v = LispVal::Float(2.5);
    let r: f64 = v.try_into().unwrap();
    assert!((r - 2.5).abs() < 1e-9);
}

#[test]
fn test_try_from_f64_from_integer() {
    let v = LispVal::Number(7);
    let r: f64 = v.try_into().unwrap();
    assert_eq!(r, 7.0);
}

#[test]
fn test_try_from_f64_err() {
    let v = LispVal::Nil;
    let r: Result<f64, LispError> = v.try_into();
    assert!(r.is_err());
}

#[test]
fn test_try_from_bool_nil_is_false() {
    let v = LispVal::Nil;
    let b: bool = v.try_into().unwrap();
    assert!(!b);
}

#[test]
fn test_try_from_bool_number_is_true() {
    let v = LispVal::Number(42);
    let b: bool = v.try_into().unwrap();
    assert!(b);
}

#[test]
fn test_try_from_string_ok() {
    let v = LispVal::String("hi".to_string());
    let s: String = v.try_into().unwrap();
    assert_eq!(s, "hi");
}

#[test]
fn test_try_from_string_err() {
    let v = LispVal::Number(1);
    let r: Result<String, LispError> = v.try_into();
    assert!(r.is_err());
}

#[test]
fn test_try_from_vec_ok() {
    let list = LispVal::from(vec![LispVal::Number(1), LispVal::Number(2)]);
    let v: Vec<LispVal> = list.try_into().unwrap();
    assert_eq!(v, vec![LispVal::Number(1), LispVal::Number(2)]);
}

#[test]
fn test_try_from_vec_nil_is_empty() {
    let v: Vec<LispVal> = LispVal::Nil.try_into().unwrap();
    assert!(v.is_empty());
}

#[test]
fn test_try_from_vec_dotted_pair_err() {
    let dotted = LispVal::Cons {
        car: Box::new(LispVal::Number(1)),
        cdr: Box::new(LispVal::Number(2)), // not a proper list
    };
    let r: Result<Vec<LispVal>, LispError> = dotted.try_into();
    assert!(r.is_err());
}

// ---------------------------------------------------------------------------
// LispVal::list helper
// ---------------------------------------------------------------------------

#[test]
fn test_list_from_i64_iter() {
    let list = LispVal::list([1i64, 2, 3]);
    let vec: Vec<LispVal> = list.try_into().unwrap();
    assert_eq!(vec, vec![LispVal::Number(1), LispVal::Number(2), LispVal::Number(3)]);
}

#[test]
fn test_list_empty() {
    let list = LispVal::list(std::iter::empty::<i64>());
    assert_eq!(list, LispVal::Nil);
}

// ---------------------------------------------------------------------------
// Helper methods
// ---------------------------------------------------------------------------

#[test]
fn test_as_number_ok() {
    assert_eq!(LispVal::Number(5).as_number(), Ok(5));
}

#[test]
fn test_as_number_err() {
    assert!(LispVal::Float(1.0).as_number().is_err());
}

#[test]
fn test_as_float_from_float() {
    let r = LispVal::Float(1.5).as_float().unwrap();
    assert_eq!(r, 1.5);
}

#[test]
fn test_as_float_coerces_integer() {
    let r = LispVal::Number(3).as_float().unwrap();
    assert_eq!(r, 3.0);
}

#[test]
fn test_as_str_val_ok() {
    let v = LispVal::String("abc".to_string());
    assert_eq!(v.as_str_val(), Ok("abc"));
}

#[test]
fn test_as_str_val_err() {
    assert!(LispVal::Number(1).as_str_val().is_err());
}

#[test]
fn test_as_list_vec_proper() {
    let list = LispVal::from(vec![LispVal::Number(10), LispVal::Number(20)]);
    let v = list.as_list_vec().unwrap();
    assert_eq!(v, vec![LispVal::Number(10), LispVal::Number(20)]);
}

#[test]
fn test_as_list_vec_nil() {
    let v = LispVal::Nil.as_list_vec().unwrap();
    assert!(v.is_empty());
}

#[test]
fn test_is_truthy_nil_false() {
    assert!(!LispVal::Nil.is_truthy());
}

#[test]
fn test_is_truthy_number_true() {
    assert!(LispVal::Number(0).is_truthy());
}

#[test]
fn test_is_truthy_string_true() {
    assert!(LispVal::String("".to_string()).is_truthy());
}

// ---------------------------------------------------------------------------
// Round-trip: i64 -> LispVal -> i64
// ---------------------------------------------------------------------------

#[test]
fn test_round_trip_i64() {
    let original: i64 = 12345;
    let val = LispVal::from(original);
    let recovered: i64 = val.try_into().unwrap();
    assert_eq!(original, recovered);
}

// ---------------------------------------------------------------------------
// Round-trip: Vec<LispVal> -> list -> Vec<LispVal>
// ---------------------------------------------------------------------------

#[test]
fn test_round_trip_vec() {
    let original = vec![LispVal::Number(1), LispVal::String("x".to_string()), LispVal::Nil];
    let list = LispVal::from(original.clone());
    let recovered: Vec<LispVal> = list.try_into().unwrap();
    assert_eq!(original, recovered);
}
