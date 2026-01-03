mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// LOGOR tests
#[test]
fn test_logor_simple() {
    let env = env_with_stdlib();
    let output = eval_line("(logor 5 3)", &env);
    assert_eq!(output, "7"); // 0101 | 0011 = 0111
}

#[test]
fn test_logor_zero() {
    let env = env_with_stdlib();
    let output = eval_line("(logor 0 0)", &env);
    assert_eq!(output, "0");
}

#[test]
fn test_logor_multiple_args() {
    let env = env_with_stdlib();
    let output = eval_line("(logor 1 2 4 8)", &env);
    assert_eq!(output, "15"); // 0001 | 0010 | 0100 | 1000 = 1111
}

#[test]
fn test_logor_no_args() {
    let env = env_with_stdlib();
    let output = eval_line("(logor)", &env);
    assert_eq!(output, "0");
}

#[test]
fn test_logor_single_arg() {
    let env = env_with_stdlib();
    let output = eval_line("(logor 42)", &env);
    assert_eq!(output, "42");
}

#[test]
fn test_logor_negative() {
    let env = env_with_stdlib();
    // -1 in two's complement has all bits set
    let output = eval_line("(logor -1 5)", &env);
    assert_eq!(output, "-1");
}

#[test]
fn test_logor_powers_of_two() {
    let env = env_with_stdlib();
    let output = eval_line("(logor 16 32 64)", &env);
    assert_eq!(output, "112"); // 16 + 32 + 64
}

// LOGAND tests
#[test]
fn test_logand_simple() {
    let env = env_with_stdlib();
    let output = eval_line("(logand 5 3)", &env);
    assert_eq!(output, "1"); // 0101 & 0011 = 0001
}

#[test]
fn test_logand_zero() {
    let env = env_with_stdlib();
    let output = eval_line("(logand 5 0)", &env);
    assert_eq!(output, "0");
}

#[test]
fn test_logand_all_bits() {
    let env = env_with_stdlib();
    let output = eval_line("(logand 15 15)", &env);
    assert_eq!(output, "15");
}

#[test]
fn test_logand_multiple_args() {
    let env = env_with_stdlib();
    let output = eval_line("(logand 15 7 3)", &env);
    assert_eq!(output, "3"); // 1111 & 0111 & 0011 = 0011
}

#[test]
fn test_logand_no_args() {
    let env = env_with_stdlib();
    let output = eval_line("(logand)", &env);
    assert_eq!(output, "-1"); // Identity for AND is all bits set
}

#[test]
fn test_logand_masking() {
    let env = env_with_stdlib();
    let output = eval_line("(logand 255 15)", &env);
    assert_eq!(output, "15"); // Extract lower 4 bits
}

#[test]
fn test_logand_negative() {
    let env = env_with_stdlib();
    let output = eval_line("(logand -1 42)", &env);
    assert_eq!(output, "42"); // -1 has all bits set, so AND with 42 gives 42
}

// LOGXOR tests
#[test]
fn test_logxor_simple() {
    let env = env_with_stdlib();
    let output = eval_line("(logxor 5 3)", &env);
    assert_eq!(output, "6"); // 0101 ^ 0011 = 0110
}

#[test]
fn test_logxor_zero() {
    let env = env_with_stdlib();
    let output = eval_line("(logxor 0 0)", &env);
    assert_eq!(output, "0");
}

#[test]
fn test_logxor_self() {
    let env = env_with_stdlib();
    let output = eval_line("(logxor 42 42)", &env);
    assert_eq!(output, "0"); // XOR with self is zero
}

#[test]
fn test_logxor_multiple_args() {
    let env = env_with_stdlib();
    let output = eval_line("(logxor 1 2 3)", &env);
    assert_eq!(output, "0"); // 001 ^ 010 ^ 011 = 000
}

#[test]
fn test_logxor_no_args() {
    let env = env_with_stdlib();
    let output = eval_line("(logxor)", &env);
    assert_eq!(output, "0");
}

#[test]
fn test_logxor_toggle_bits() {
    let env = env_with_stdlib();
    let output = eval_line("(logxor 15 5)", &env);
    assert_eq!(output, "10"); // 1111 ^ 0101 = 1010
}

// LEFTSHIFT tests
#[test]
fn test_leftshift_positive() {
    let env = env_with_stdlib();
    let output = eval_line("(leftshift 1 3)", &env);
    assert_eq!(output, "8"); // 1 << 3 = 8
}

#[test]
fn test_leftshift_zero_shift() {
    let env = env_with_stdlib();
    let output = eval_line("(leftshift 5 0)", &env);
    assert_eq!(output, "5");
}

#[test]
fn test_leftshift_right_negative() {
    let env = env_with_stdlib();
    let output = eval_line("(leftshift 8 -3)", &env);
    assert_eq!(output, "1"); // 8 >> 3 = 1
}

#[test]
fn test_leftshift_multiply_by_two() {
    let env = env_with_stdlib();
    let output = eval_line("(leftshift 7 1)", &env);
    assert_eq!(output, "14"); // 7 * 2 = 14
}

#[test]
fn test_leftshift_divide_by_two() {
    let env = env_with_stdlib();
    let output = eval_line("(leftshift 14 -1)", &env);
    assert_eq!(output, "7"); // 14 / 2 = 7
}

#[test]
fn test_leftshift_large_shift() {
    let env = env_with_stdlib();
    let output = eval_line("(leftshift 1 10)", &env);
    assert_eq!(output, "1024"); // 1 << 10 = 1024
}

#[test]
fn test_leftshift_large_right_shift() {
    let env = env_with_stdlib();
    let output = eval_line("(leftshift 1024 -10)", &env);
    assert_eq!(output, "1"); // 1024 >> 10 = 1
}

#[test]
fn test_leftshift_zero_value() {
    let env = env_with_stdlib();
    let output = eval_line("(leftshift 0 5)", &env);
    assert_eq!(output, "0");
}

// Combined bitwise operations
#[test]
fn test_combined_set_bit() {
    let env = env_with_stdlib();
    // Set bit 3 in value 0: value | (1 << 3)
    let output = eval_line("(logor 0 (leftshift 1 3))", &env);
    assert_eq!(output, "8");
}

#[test]
fn test_combined_clear_bit() {
    let env = env_with_stdlib();
    // Clear bit 1 in value 7: value & ~(1 << 1)
    // We don't have NOT but can use XOR with all bits set
    let output = eval_line("(logand 7 (logxor -1 (leftshift 1 1)))", &env);
    assert_eq!(output, "5"); // 0111 & ~0010 = 0101
}

#[test]
fn test_combined_toggle_bit() {
    let env = env_with_stdlib();
    // Toggle bit 2 in value 5: value ^ (1 << 2)
    let output = eval_line("(logxor 5 (leftshift 1 2))", &env);
    assert_eq!(output, "1"); // 0101 ^ 0100 = 0001
}

#[test]
fn test_combined_extract_bits() {
    let env = env_with_stdlib();
    // Extract bits 2-4 from 0b11110101 (245): (value >> 2) & 0b111
    let output = eval_line("(logand (leftshift 245 -2) 7)", &env);
    assert_eq!(output, "5"); // Extract 101 from ...11010...
}

#[test]
fn test_bitwise_mask_creation() {
    let env = env_with_stdlib();
    // Create a mask with lower 4 bits set: (1 << 4) - 1 = 15
    eval_line("(def mask (+ (leftshift 1 4) -1))", &env);
    let output = eval_line("mask", &env);
    assert_eq!(output, "15");
}

#[test]
fn test_all_operations_combined() {
    let env = env_with_stdlib();
    // Complex: ((5 | 3) & 15) ^ 2 << 1
    let output = eval_line("(leftshift (logxor (logand (logor 5 3) 15) 2) 1)", &env);
    assert_eq!(output, "10"); // ((7 & 15) ^ 2) << 1 = (7 ^ 2) << 1 = 5 << 1 = 10
}
