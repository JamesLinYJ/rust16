// State 40 is the next paragraph, 42 the current module and 46 the relocation count.
fn entryname(p: u16, n: u16) -> bool {
    return n == 6
        && fetch(0, p) == 95
        && fetch(0, p + 1) == 115
        && fetch(0, p + 2) == 116
        && fetch(0, p + 3) == 97
        && fetch(0, p + 4) == 114
        && fetch(0, p + 5) == 116;
}
fn getidx() -> u16 {
    let p: u16 = st(60);
    if p >= st(62) {
        fail(50);
    }
    let a: u16 = fetch(0, p);
    set(60, p + 1);
    if a < 128 {
        return a;
    }
    if p + 1 >= st(62) {
        fail(50);
    }
    set(60, p + 2);
    return (a & 127) * 256 + fetch(0, p + 1);
}
// Two 60 KiB banks hold the segmented load image. Normalize paragraph:offset
// before choosing the backing bank, including fixups across a bank boundary.
fn image_byte(base: u16, at: u16, value: u16) -> u16 {
    let para: u16 = base + at / 16;
    if para < 3840 { save(1, para * 16 + at % 16, value); }
    else { save(6, (para - 3840) * 16 + at % 16, value); }
    return 0;
}
fn image_word(base:u16, at:u16, value:u16)->u16 {
    image_byte(base, at, value % 256);
    image_byte(base, at + 1, value / 256);
    return 0;
}
fn module(ai: u16, pass: u16) -> u16 {
    let size: u16 = input(ai, 0);
    let mut p: u16 = 0;
    let mut ex: u16 = 0;
    let mut ns: u16 = 0;
    let mut last: u16 = 0;
    let mut count: u16 = 0;
    let base: u16 = st(40);
    let mut length: u16 = 0;
    let mut done: bool = false;
    while p < size {
        if done || size - p < 4 {
            fail(50);
        }
        let kind: u16 = fetch(0, p);
        let n: u16 = word(0, p + 1);
        if n == 0 || n > size - p - 3 {
            fail(50);
        }
        let end: u16 = p + 2 + n;
        let mut q: u16 = p;
        let mut sum: u16 = 0;
        while q <= end {
            sum = sum + fetch(0, q);
            q = q + 1;
        }
        if (sum & 255) != 0 {
            fail(51);
        }
        q = p + 3;
        set(62, end);
        if kind == 152 {
            if ns != 0 || n < 7 || (fetch(0, q) & 227) != 96 {
                fail(52);
            }
            length = word(0, q + 1);
            if length > 65000 || base + (length + 15) / 16 > 7680 {
                fail(53);
            }
            ns = 1;
        } else if kind == 140 {
            while q < end {
                let k: u16 = fetch(0, q);
                if k == 0 || k >= end - q {
                    fail(50);
                }
                let id: u16 = intern(0, q + 1, k);
                ex = ex + 1;
                if ex >= 4000 {
                    fail(53);
                }
                setw(5, ex * 2, id);
                set(60, q + k + 1);
                getidx();
                q = st(60);
            }
        } else if kind == 144 {
            set(60, q);
            if getidx() != 0 || getidx() != 1 || ns != 1 {
                fail(52);
            }
            q = st(60);
            while q < end {
                let k: u16 = fetch(0, q);
                if k == 0 || k + 4 > end - q {
                    fail(50);
                }
                let ofs: u16 = word(0, q + 1 + k);
                if ofs > length {
                    fail(53);
                }
                if pass == 0 {
                    let id: u16 = intern(0, q + 1, k);
                    if word(3, id * 6 + 2) != 0 {
                        fail(54);
                    }
                    setw(3, id * 6 + 2, 1);
                    setw(3, id * 6 + 4, ofs);
                    setw(4, id * 2, base);
                    if entryname(q + 1, k) {
                        set(50, 1);
                        set(52, base);
                        set(54, ofs);
                    }
                }
                set(60, q + k + 3);
                getidx();
                q = st(60);
            }
        } else if kind == 160 {
            set(60, q);
            if getidx() != 1 || ns != 1 {
                fail(52);
            }
            q = st(60);
            if end - q < 2 {
                fail(50);
            }
            last = word(0, q);
            count = end - q - 2;
            if last > length || count > length - last {
                fail(53);
            }
            if pass == 1 {
                let mut k: u16 = 0;
                while k < count { image_byte(base, last+k, fetch(0,q+2+k)); k=k+1; }
            }
        } else if kind == 156 {
            while q < end {
                if end - q < 4 || (fetch(0, q) & 252) != 204 || fetch(0, q + 2) != 86 {
                    fail(55);
                }
                let d: u16 = (fetch(0, q) & 3) * 256 + fetch(0, q + 1);
                if d > count || count - d < 4 {
                    fail(55);
                }
                set(60, q + 3);
                let ix: u16 = getidx();
                if ix == 0 || ix > ex {
                    fail(55);
                }
                q = st(60);
                if pass == 1 {
                    let id: u16 = word(5, ix * 2);
                    if word(3, id * 6 + 2) == 0 {
                        fail(56);
                    }
                    let at: u16 = last + d;
                    image_word(base, at, word(3, id * 6 + 4));
                    image_word(base, at + 2, word(4, id * 2));
                    let ri: u16 = st(46);
                    if ri >= 4000 {
                        fail(53);
                    }
                    setw(9, ri * 4, at + 2);
                    setw(9, ri * 4 + 2, base);
                    set(46, ri + 1);
                }
            }
        } else if kind == 138 {
            done = true;
        } else if kind != 128 && kind != 150 && kind != 136 {
            fail(57);
        }
        p = end + 1;
    }
    if !done || ns != 1 {
        fail(50);
    }
    set(40, base + (length + 15) / 16);
    return 0;
}
pub fn main16() -> u16 {
    let mut ai: u16 = 1;
    while arg(ai, 0) != 0 && arg(ai, 0) != 45 {
        ai = ai + 1;
    }
    if ai == 1
        || arg(ai, 0) != 45
        || arg(ai, 1) != 111
        || arg(ai, 2) != 0
        || arg(ai + 1, 0) == 0
        || arg(ai + 2, 0) != 0
    {
        fail(43);
    }
    let mut pass: u16 = 0;
    while pass < 2 {
        set(40, 0);
        let mut i: u16 = 1;
        while i < ai {
            module(i, pass);
            i = i + 1;
        }
        pass = pass + 1;
    }
    if st(50) == 0 {
        fail(58);
    }
    let n: u16 = st(46);
    let hdr: u16 = (28 + n * 4 + 15) / 16;
    let img: u16 = st(40);
    let mut clear: u16 = 0;
    while clear < hdr*16 { save(0,clear,0); clear=clear+1; }
    let paras:u16=hdr+img;
    setw(0,0,23117);
    setw(0,2,(paras % 32)*16);
    setw(0,4,(paras+31)/32);
    setw(0,6,n); setw(0,8,hdr); setw(0,10,1024); setw(0,12,1024);
    setw(0,14,img); setw(0,16,16384); setw(0,20,st(54)); setw(0,22,st(52)); setw(0,24,28);
    moveb(9,0,0,28,n*4);
    output(ai+1,0,hdr*16);
    if img > 3840 {append(ai+1,1,61440);append(ai+1,6,(img-3840)*16);}
    else {append(ai+1,1,img*16);}
    return 0;
}
