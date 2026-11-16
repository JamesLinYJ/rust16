// Byte storage uses separate DOS segments. All offsets are checked by the runtime.
fn word(b: u16, p: u16) -> u16 {
    return fetch(b, p) | (fetch(b, p + 1) << 8);
}
fn setw(b: u16, p: u16, v: u16) -> u16 {
    save(b, p, v);
    save(b, p + 1, v >> 8);
    return v;
}
fn st(p: u16) -> u16 {
    return word(8, p);
}
fn set(p: u16, v: u16) -> u16 {
    return setw(8, p, v);
}
fn moveb(a: u16, p: u16, b: u16, q: u16, n: u16) -> u16 {
    let mut i: u16 = 0;
    while i < n {
        save(b, q + i, fetch(a, p + i));
        i = i + 1;
    }
    return i;
}
fn eqb(a: u16, p: u16, b: u16, q: u16, n: u16) -> bool {
    let mut i: u16 = 0;
    while i < n {
        if fetch(a, p + i) != fetch(b, q + i) {
            return false;
        }
        i = i + 1;
    }
    return true;
}
fn digit(c: u16) -> bool {
    return c >= 48 && c <= 57;
}
fn letter(c: u16) -> bool {
    return (c >= 65 && c <= 90) || (c >= 97 && c <= 122) || c == 95;
}
fn number(n: u16) -> u16 {
    if n >= 10 {
        number(n / 10);
    }
    putch(48 + n % 10);
    return 0;
}
// Interned names are length-prefixed, with independent definition and lookup.
fn intern(b: u16, p: u16, n: u16) -> u16 {
    let mut id: u16 = 1;
    let mut pos: u16 = 0;
    while id <= st(0) {
        pos = word(3, id * 6);
        if fetch(2, pos) == n && eqb(2, pos + 1, b, p, n) {
            return id;
        }
        id = id + 1;
    }
    if n == 0 || n > 127 || id >= 1500 || st(2) > 12000 - n - 1 {
        fail(20);
    }
    pos = st(2);
    save(2, pos, n);
    moveb(b, p, 2, pos + 1, n);
    setw(3, id * 6, pos);
    set(2, pos + 1 + n);
    set(0, id);
    return id;
}
fn emit(v: u16) -> u16 {
    let p: u16 = st(4);
    if p >= 59900 {
        fail(21);
    }
    save(1, p, v);
    set(4, p + 1);
    return p;
}
fn emitw(v: u16) -> u16 {
    let p: u16 = emit(v);
    emit(v >> 8);
    return p;
}
fn patch(p: u16, d: u16) -> u16 {
    return setw(1, p, d - p - 2);
}
fn define(id: u16) -> u16 {
    if word(3, id * 6 + 2) != 0 {
        fail(22);
    }
    setw(3, id * 6 + 2, 1);
    return setw(3, id * 6 + 4, st(4));
}
fn call16(id: u16) -> u16 {
    emit(154);
    let pos: u16 = emitw(0);
    emitw(0);
    let i: u16 = st(6);
    if i >= 3900 {
        fail(23);
    }
    setw(5, i * 4, pos);
    setw(5, i * 4 + 2, id);
    set(6, i + 1);
    return pos;
}
fn recbyte(v: u16) -> u16 {
    let p: u16 = st(10);
    if p >= 1023 {
        fail(24);
    }
    save(7, p, v);
    set(10, p + 1);
    return p;
}
fn recword(v: u16) -> u16 {
    recbyte(v);
    return recbyte(v >> 8);
}
fn idx(v: u16) -> u16 {
    if v >= 128 {
        recbyte(128 | (v >> 8));
    }
    return recbyte(v);
}
fn outbyte(v: u16) -> u16 {
    let p: u16 = st(12);
    if p >= 65500 {
        fail(25);
    }
    save(6, p, v);
    set(12, p + 1);
    return p;
}
fn record(kind: u16) -> u16 {
    let n: u16 = st(10) + 1;
    let mut s: u16 = kind + (n & 255) + (n >> 8);
    outbyte(kind);
    outbyte(n);
    outbyte(n >> 8);
    let mut i: u16 = 0;
    while i + 1 < n {
        let c: u16 = fetch(7, i);
        s = s + c;
        outbyte(c);
        i = i + 1;
    }
    outbyte(0 - s);
    set(10, 0);
    return 0;
}
fn recname(id: u16) -> u16 {
    let p: u16 = word(3, id * 6);
    let n: u16 = fetch(2, p);
    let mut i: u16 = 0;
    while i <= n {
        recbyte(fetch(2, p + i));
        i = i + 1;
    }
    return 0;
}
fn obj() -> u16 {
    recbyte(1);
    recbyte(77);
    record(128); // THEADR "M"
    recbyte(4);
    recbyte(67);
    recbyte(79);
    recbyte(68);
    recbyte(69);
    record(150);
    recbyte(96);
    recword(st(4));
    idx(1);
    idx(1);
    idx(1);
    record(152); // paragraph-aligned private segment
    let mut i: u16 = 1;
    while i <= st(0) {
        recname(i);
        idx(0);
        record(140);
        if word(3, i * 6 + 2) != 0 {
            idx(0);
            idx(1);
            recname(i);
            recword(word(3, i * 6 + 4));
            idx(0);
            record(144);
        }
        i = i + 1;
    }
    let mut p: u16 = 0;
    let mut f: u16 = 0;
    while p < st(4) {
        let mut n: u16 = 256;
        if st(4) - p < n {
            n = st(4) - p;
        }
        let mut j: u16 = f;
        while j < st(6) && word(5, j * 4) < p + n {
            if word(5, j * 4) + 4 > p + n {
                n = word(5, j * 4) - p;
            }
            j = j + 1;
        }
        idx(1);
        recword(p);
        i = 0;
        while i < n {
            recbyte(fetch(1, p + i));
            i = i + 1;
        }
        record(160);
        while f < st(6) && word(5, f * 4) < p + n {
            let d: u16 = word(5, f * 4) - p;
            recbyte(204 | (d >> 8));
            recbyte(d);
            recbyte(86);
            idx(word(5, f * 4 + 2));
            f = f + 1;
        }
        if st(10) != 0 {
            record(156);
        }
        p = p + n;
    }
    recbyte(0);
    record(138);
    return st(12);
}
