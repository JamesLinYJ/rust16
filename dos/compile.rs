fn keyword(p: u16, n: u16) -> u16 {
    if n == 2 && fetch(0, p + 0) == 102 && fetch(0, p + 1) == 110 {
        return 300;
    }
    if n == 3 && fetch(0, p + 0) == 108 && fetch(0, p + 1) == 101 && fetch(0, p + 2) == 116 {
        return 301;
    }
    if n == 3 && fetch(0, p + 0) == 109 && fetch(0, p + 1) == 117 && fetch(0, p + 2) == 116 {
        return 302;
    }
    if n == 6
        && fetch(0, p + 0) == 114
        && fetch(0, p + 1) == 101
        && fetch(0, p + 2) == 116
        && fetch(0, p + 3) == 117
        && fetch(0, p + 4) == 114
        && fetch(0, p + 5) == 110
    {
        return 303;
    }
    if n == 2 && fetch(0, p + 0) == 105 && fetch(0, p + 1) == 102 {
        return 304;
    }
    if n == 4
        && fetch(0, p + 0) == 101
        && fetch(0, p + 1) == 108
        && fetch(0, p + 2) == 115
        && fetch(0, p + 3) == 101
    {
        return 305;
    }
    if n == 5
        && fetch(0, p + 0) == 119
        && fetch(0, p + 1) == 104
        && fetch(0, p + 2) == 105
        && fetch(0, p + 3) == 108
        && fetch(0, p + 4) == 101
    {
        return 306;
    }
    if n == 3 && fetch(0, p + 0) == 117 && fetch(0, p + 1) == 49 && fetch(0, p + 2) == 54 {
        return 307;
    }
    if n == 4
        && fetch(0, p + 0) == 98
        && fetch(0, p + 1) == 111
        && fetch(0, p + 2) == 111
        && fetch(0, p + 3) == 108
    {
        return 308;
    }
    if n == 4
        && fetch(0, p + 0) == 116
        && fetch(0, p + 1) == 114
        && fetch(0, p + 2) == 117
        && fetch(0, p + 3) == 101
    {
        return 309;
    }
    if n == 5
        && fetch(0, p + 0) == 102
        && fetch(0, p + 1) == 97
        && fetch(0, p + 2) == 108
        && fetch(0, p + 3) == 115
        && fetch(0, p + 4) == 101
    {
        return 310;
    }
    if n == 3 && fetch(0, p + 0) == 112 && fetch(0, p + 1) == 117 && fetch(0, p + 2) == 98 {
        return 311;
    }
    if n == 2 && fetch(0, p + 0) == 97 && fetch(0, p + 1) == 115 {
        return 312;
    }
    return 256;
}
fn err(code: u16) -> u16 {
    let mut line: u16 = 1;
    let mut i: u16 = 0;
    while i < st(22) {
        if fetch(0, i) == 10 {
            line = line + 1;
        }
        i = i + 1;
    }
    number(line);
    putch(58);
    number(code);
    putch(13);
    putch(10);
    return fail(code);
}
fn lex() -> u16 {
    let mut p: u16 = st(18);
    let mut c: u16 = fetch(0, p);
    while p < st(16) && (c <= 32 || (c == 47 && fetch(0, p + 1) == 47)) {
        if c == 47 {
            while p < st(16) && fetch(0, p) != 10 {
                p = p + 1;
            }
        } else {
            p = p + 1;
        }
        c = fetch(0, p);
    }
    set(22, p);
    let mut k: u16 = c;
    let mut e: u16 = p + 1;
    if p >= st(16) {
        k = 0;
        e = p;
    } else if letter(c) {
        while e < st(16) && (letter(fetch(0, e)) || digit(fetch(0, e))) {
            e = e + 1;
        }
        k = keyword(p, e - p);
    } else if digit(c) {
        let mut v: u16 = 0;
        e = p;
        while e < st(16) && digit(fetch(0, e)) {
            let d: u16 = fetch(0, e) - 48;
            if v > 6553 || (v == 6553 && d > 5) {
                err(30);
            }
            v = v * 10 + d;
            e = e + 1;
        }
        k = 257;
        set(26, v);
    } else {
        let d: u16 = fetch(0, e);
        if ((c == 61 || c == 33 || c == 60 || c == 62) && d == 61)
            || (c == 45 && d == 62)
            || ((c == 38 || c == 124 || c == 60 || c == 62) && c == d)
        {
            k = c + d * 256;
            e = e + 1;
        }
    }
    set(24, e - p);
    set(18, e);
    set(20, k);
    return k;
}
fn tok() -> u16 {
    return st(20);
}
fn take(k: u16) -> u16 {
    if tok() != k {
        err(31);
    }
    return lex();
}
fn name() -> u16 {
    if tok() != 256 {
        err(32);
    }
    let id: u16 = intern(0, st(22), st(24));
    lex();
    return id;
}
fn typ() -> u16 {
    let t: u16 = tok();
    if t != 307 && t != 308 {
        err(33);
    }
    lex();
    return t;
}
fn local(id: u16) -> u16 {
    let mut i: u16 = st(28);
    while i != 0 {
        i = i - 1;
        if word(4, i * 8) == id {
            return i + 1;
        }
    }
    return 0;
}
fn bind(id: u16, offset: u16, mutable: u16, t: u16) -> u16 {
    let i: u16 = st(28);
    if i >= 128 {
        err(34);
    }
    setw(4, i * 8, id);
    setw(4, i * 8 + 2, offset);
    setw(4, i * 8 + 4, mutable);
    setw(4, i * 8 + 6, t);
    set(28, i + 1);
    return i;
}
fn mem(op: u16, i: u16) -> u16 {
    emit(op);
    emit(134);
    return emitw(word(4, (i - 1) * 8 + 2));
}
fn prec(k: u16) -> u16 {
    if k == 31868 {
        return 1;
    }
    if k == 9766 {
        return 2;
    }
    if k == 15677 || k == 15649 || k == 15676 || k == 15678 || k == 60 || k == 62 {
        return 3;
    }
    if k == 124 {
        return 4;
    }
    if k == 94 {
        return 5;
    }
    if k == 38 {
        return 6;
    }
    if k == 15420 || k == 15934 {
        return 7;
    }
    if k == 43 || k == 45 {
        return 8;
    }
    if k == 42 || k == 47 || k == 37 {
        return 9;
    }
    return 0;
}
fn boolean(cc: u16) -> u16 {
    if st(34) == 386 {
        emit(15);
        emit(cc + 32);
        emit(192);
        emit(48);
        emit(228);
        return 308;
    }
    emit(cc);
    emit(4);
    emit(49);
    emit(192);
    emit(235);
    emit(3);
    emit(184);
    emitw(1);
    return 308;
}
fn falsejump() -> u16 {
    emit(9);
    emit(192);
    emit(117);
    emit(3);
    emit(233);
    return emitw(0);
}
fn binary(k: u16) -> u16 {
    emit(137);
    emit(193);
    emit(88);
    if k == 43 {
        emit(1);
        emit(200);
    } else if k == 45 {
        emit(41);
        emit(200);
    } else if k == 42 {
        emit(247);
        emit(225);
    } else if k == 47 || k == 37 {
        emit(49);
        emit(210);
        emit(247);
        emit(241);
        if k == 37 {
            emit(137);
            emit(208);
        }
    } else if k == 38 {
        emit(33);
        emit(200);
    } else if k == 124 {
        emit(9);
        emit(200);
    } else if k == 94 {
        emit(49);
        emit(200);
    } else if k == 15420 || k == 15934 {
        emit(128);
        emit(225);
        emit(15);
        emit(211);
        if k == 15420 {
            emit(224);
        } else {
            emit(232);
        }
    } else {
        emit(57);
        emit(200);
        if k == 15677 {
            return boolean(116);
        }
        if k == 15649 {
            return boolean(117);
        }
        if k == 60 {
            return boolean(114);
        }
        if k == 62 {
            return boolean(119);
        }
        if k == 15676 {
            return boolean(118);
        }
        return boolean(115);
    }
    return 307;
}
fn primary() -> u16 {
    let k: u16 = tok();
    if k == 257 || k == 309 || k == 310 {
        let mut v: u16 = st(26);
        if k == 309 {
            v = 1;
        }
        if k == 310 {
            v = 0;
        }
        lex();
        emit(184);
        emitw(v);
        if k == 257 {
            return 307;
        }
        return 308;
    }
    if k == 40 {
        lex();
        let t: u16 = expr(1);
        take(41);
        return t;
    }
    if k == 33 {
        lex();
        let t: u16 = primary();
        if t == 308 {
            emit(131);
            emit(240);
            emit(1);
        } else {
            emit(247);
            emit(208);
        }
        return t;
    }
    let id: u16 = name();
    if tok() == 40 {
        lex();
        let mut n: u16 = 0;
        while tok() != 41 {
            let actual: u16 = expr(1);
            if n >= 16 {
                err(36);
            }
            if word(9, id * 4) != 0 {
                let mask: u16 = word(9, 6000 + id * 2);
                if (actual == 308) != ((mask & (1 << n)) != 0) {
                    err(38);
                }
            }
            emit(80);
            n = n + 1;
            if tok() != 44 {
                if tok() != 41 {
                    err(35);
                }
            } else {
                lex();
            }
        }
        lex();
        call16(id);
        if n > 0 {
            emit(129);
            emit(196);
            emitw(n * 2);
        }
        let sig: u16 = word(9, id * 4);
        if sig != 0 && sig != n + 1 {
            err(36);
        }
        let t: u16 = word(9, id * 4 + 2);
        if t == 0 {
            return 307;
        }
        return t;
    }
    let i: u16 = local(id);
    if i == 0 {
        err(37);
    }
    mem(139, i);
    return word(4, (i - 1) * 8 + 6);
}
fn expr(level: u16) -> u16 {
    let mut t: u16 = primary();
    while prec(tok()) >= level {
        let k: u16 = tok();
        let p: u16 = prec(k);
        lex();
        if k == 9766 || k == 31868 {
            if t != 308 {
                err(38);
            }
            if k == 31868 {
                emit(131);
                emit(240);
                emit(1);
            }
            let next: u16 = falsejump();
            if expr(p + 1) != 308 {
                err(38);
            }
            emit(233);
            let end: u16 = emitw(0);
            patch(next, st(4));
            emit(184);
            if k == 31868 {
                emitw(1);
            } else {
                emitw(0);
            }
            patch(end, st(4));
        } else {
            emit(80);
            let rhs: u16 = expr(p + 1);
            if rhs != t {
                err(38);
            }
            if t == 308 && p > 3 && (k != 38 && k != 94 && k != 124) {
                err(38);
            }
            let left_type: u16 = t;
            t = binary(k);
            if left_type == 308 {
                t = 308;
            }
        }
    }
    return t;
}
fn ret16() -> u16 {
    if st(34) >= 286 {
        emit(201);
    } else {
        emit(137);
        emit(236);
        emit(93);
    }
    emit(203);
    return 1;
}
fn block() -> u16 {
    take(123);
    let was: u16 = st(28);
    let mut returns: u16 = 0;
    while tok() != 125 {
        returns = returns | stmt();
    }
    lex();
    set(28, was);
    return returns;
}
fn stmt() -> u16 {
    if tok() == 301 {
        lex();
        let mut m: u16 = 0;
        if tok() == 302 {
            lex();
            m = 1;
        }
        let id: u16 = name();
        take(58);
        let t: u16 = typ();
        take(61);
        let r: u16 = expr(1);
        if r != t {
            err(38);
        }
        let size: u16 = st(36) + 2;
        set(36, size);
        let i: u16 = bind(id, 0 - size, m, t) + 1;
        mem(137, i);
        take(59);
        return 0;
    }
    if tok() == 303 {
        lex();
        let t: u16 = expr(1);
        if t != st(32) {
            err(38);
        }
        take(59);
        return ret16();
    }
    if tok() == 304 {
        lex();
        if expr(1) != 308 {
            err(38);
        }
        let a: u16 = falsejump();
        let yes: u16 = block();
        if tok() == 305 {
            lex();
            emit(233);
            let b: u16 = emitw(0);
            patch(a, st(4));
            let mut no: u16 = 0;
            if tok() == 304 {
                no = stmt();
            } else {
                no = block();
            }
            patch(b, st(4));
            return yes & no;
        } else {
            patch(a, st(4));
        }
        return 0;
    }
    if tok() == 306 {
        lex();
        let a: u16 = st(4);
        if expr(1) != 308 {
            err(38);
        }
        let b: u16 = falsejump();
        block();
        emit(233);
        patch(emitw(0), a);
        patch(b, st(4));
        return 0;
    }
    if tok() == 256 {
        let pos: u16 = st(22);
        let id: u16 = name();
        if tok() == 61 {
            lex();
            let i: u16 = local(id);
            if i == 0 || word(4, (i - 1) * 8 + 4) == 0 {
                err(39);
            }
            if expr(1) != word(4, (i - 1) * 8 + 6) {
                err(38);
            }
            mem(137, i);
            take(59);
            return 0;
        }
        set(18, pos);
        lex();
    }
    expr(1);
    take(59);
    return 0;
}
fn function(pass: u16) -> u16 {
    if tok() == 311 {
        lex();
    }
    take(300);
    let id: u16 = name();
    take(40);
    let mut n: u16 = 0;
    let mut bool_args: u16 = 0;
    set(28, 0);
    while tok() != 41 {
        let a: u16 = name();
        take(58);
        let t: u16 = typ();
        if n >= 16 || local(a) != 0 {
            err(36);
        }
        if t == 308 {
            bool_args = bool_args | (1 << n);
        }
        bind(a, 0, 0, t);
        n = n + 1;
        if tok() == 44 {
            lex();
        } else if tok() != 41 {
            err(40);
        }
    }
    lex();
    take(15917);
    let ret: u16 = typ();
    set(32, ret);
    if pass == 0 {
        if word(9, id * 4) != 0 {
            err(22);
        }
        setw(9, 6000 + id * 2, bool_args);
        setw(9, id * 4, n + 1);
        setw(9, id * 4 + 2, ret);
        take(123);
        let mut depth: u16 = 1;
        while depth > 0 {
            if tok() == 0 {
                err(41);
            }
            if tok() == 123 {
                depth = depth + 1;
            }
            if tok() == 125 {
                depth = depth - 1;
            }
            lex();
        }
        return 0;
    }
    let mut i: u16 = 0;
    while i < n {
        setw(4, i * 8 + 2, 6 + (n - i - 1) * 2);
        i = i + 1;
    }
    define(id);
    set(36, 0);
    let mut frame: u16 = 0;
    if st(34) >= 286 {
        emit(200);
        frame = emitw(0);
        emit(0);
    } else {
        emit(85);
        emit(137);
        emit(229);
        emit(129);
        emit(236);
        frame = emitw(0);
    }
    if block() == 0 {
        err(44);
    }
    setw(1, frame, st(36));
    return 0;
}
pub fn main16() -> u16 {
    let mut ai: u16 = 1;
    let mut cpu: u16 = 0;
    if arg(ai, 0) == 45 && arg(ai, 1) == 109 {
        let c: u16 = arg(ai, 2);
        if c == 50 && arg(ai, 3) == 56 && arg(ai, 4) == 54 && arg(ai, 5) == 0 {
            cpu = 286;
        } else if c == 51 && arg(ai, 3) == 56 && arg(ai, 4) == 54 && arg(ai, 5) == 0 {
            cpu = 386;
        } else if c != 56
            || arg(ai, 3) != 48
            || arg(ai, 4) != 56
            || (arg(ai, 5) != 54 && arg(ai, 5) != 56)
            || arg(ai, 6) != 0
        {
            err(42);
        }
        ai = ai + 1;
    }
    set(34, cpu);
    if arg(ai, 0) == 0
        || arg(ai + 1, 0) != 45
        || arg(ai + 1, 1) != 111
        || arg(ai + 1, 2) != 0
        || arg(ai + 2, 0) == 0
        || arg(ai + 3, 0) != 0
    {
        err(43);
    }
    set(16, input(ai, 0));
    let mut pass: u16 = 0;
    while pass < 2 {
        set(18, 0);
        lex();
        while tok() != 0 {
            function(pass);
        }
        pass = pass + 1;
    }
    output(ai + 2, 6, obj());
    return 0;
}
