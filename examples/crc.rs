fn decimal(value: u16) -> u16 {
    if value >= 10 {
        decimal(value / 10);
    }
    putch(48 + value % 10);
    return 0;
}
pub fn main16() -> u16 {
    let size: u16 = input(1, 0);
    let mut crc: u16 = 65535;
    let mut i: u16 = 0;
    while i < size {
        crc = crc ^ (fetch(0, i) << 8);
        let mut bit: u16 = 0;
        while bit < 8 {
            if (crc & 32768) != 0 {
                crc = (crc << 1) ^ 4129;
            } else {
                crc = crc << 1;
            }
            bit = bit + 1;
        }
        i = i + 1;
    }
    decimal(crc);
    putch(13);
    putch(10);
    return 0;
}
