fn triangle(n: u16) -> u16 {
    let mut i: u16 = 0;
    let mut sum: u16 = 0;
    while i < n {
        i = i + 1;
        sum = sum + i;
    }
    return sum;
}

pub fn main16() -> u16 {
    let sum: u16 = triangle(12);
    putch(48 + sum / 10);
    putch(48 + sum % 10);
    putch(13);
    putch(10);
    if sum != 78 { return 1; }
    return 0;
}
