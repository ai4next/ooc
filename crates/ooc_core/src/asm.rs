//! OC-1 assembler: text ⇄ [`Instr`]. Content (starter code, reference
//! solutions) is authored as assembly so it reads like a real listing.

use crate::isa::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AsmErrorKind {
    UnknownOp(String),
    BadOperand(String),
    Arity { want: usize, got: usize },
    Shape(ShapeError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsmError {
    /// 0-based source line (for shape errors: the instruction's line).
    pub line: usize,
    pub kind: AsmErrorKind,
}

fn special(s: &str) -> Option<Special> {
    Special::ALL
        .into_iter()
        .chain([Special::Clock])
        .chain(Special::GRID)
        .find(|sp| sp.name() == s)
}

fn int(s: &str) -> Option<i32> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let v = if let Some(h) = body.strip_prefix("0x") {
        i64::from_str_radix(h, 16).ok()?
    } else {
        body.parse::<i64>().ok()?
    };
    let v = if neg { -v } else { v };
    i32::try_from(v).ok()
}

fn reg(s: &str) -> Option<u8> {
    s.strip_prefix('r')?.parse().ok().filter(|r| *r < REGS)
}

fn pred(s: &str) -> Option<u8> {
    s.strip_prefix('p')?.parse().ok().filter(|p| *p < PREDS)
}

fn src(s: &str) -> Option<Src> {
    if let Some(r) = reg(s) {
        return Some(Src::Reg(r));
    }
    if let Some(sp) = special(s) {
        return Some(Src::Sp(sp));
    }
    int(s).map(Src::Imm)
}

fn mem(s: &str) -> Option<Src> {
    src(s.strip_prefix('[')?.strip_suffix(']')?.trim())
}

/// Parses one instruction line (already stripped of comments, lowercase ops ok).
pub fn parse_line(text: &str) -> Result<Option<Instr>, AsmErrorKind> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let (op, rest) = text.split_once(char::is_whitespace).unwrap_or((text, ""));
    let op = op.to_ascii_uppercase();
    let args: Vec<String> = if rest.trim().is_empty() {
        Vec::new()
    } else {
        rest.split(',')
            .map(|a| a.trim().to_ascii_lowercase())
            .collect()
    };
    let want = |n: usize| {
        if args.len() == n {
            Ok(())
        } else {
            Err(AsmErrorKind::Arity {
                want: n,
                got: args.len(),
            })
        }
    };
    let bad = |a: &str| AsmErrorKind::BadOperand(a.to_string());
    let r = |i: usize| reg(&args[i]).ok_or_else(|| bad(&args[i]));
    let p = |i: usize| pred(&args[i]).ok_or_else(|| bad(&args[i]));
    let s = |i: usize| src(&args[i]).ok_or_else(|| bad(&args[i]));
    let m = |i: usize| mem(&args[i]).ok_or_else(|| bad(&args[i]));

    if let Some(c) = op.strip_prefix("SETP.") {
        let cmp = Cmp::ALL
            .into_iter()
            .find(|k| k.name() == c)
            .ok_or_else(|| AsmErrorKind::UnknownOp(op.clone()))?;
        want(3)?;
        return Ok(Some(Instr::SetP {
            cmp,
            p: p(0)?,
            a: s(1)?,
            b: s(2)?,
        }));
    }
    if let Some(cvt) = Cvt::ALL.into_iter().find(|c| c.name() == op) {
        want(2)?;
        return Ok(Some(Instr::Cvt {
            op: cvt,
            d: r(0)?,
            a: s(1)?,
        }));
    }
    if let Some(alu) = Alu::ALL.into_iter().chain(Alu::FP).find(|a| a.name() == op) {
        want(3)?;
        return Ok(Some(Instr::Alu {
            op: alu,
            d: r(0)?,
            a: s(1)?,
            b: s(2)?,
        }));
    }
    let ins = match op.as_str() {
        "MOV" => {
            want(2)?;
            Instr::Mov { d: r(0)?, a: s(1)? }
        }
        "SEL" => {
            want(4)?;
            Instr::Sel {
                d: r(0)?,
                p: p(1)?,
                a: s(2)?,
                b: s(3)?,
            }
        }
        "IF" => {
            want(1)?;
            Instr::If { p: p(0)? }
        }
        "ELSE" => {
            want(0)?;
            Instr::Else
        }
        "ENDIF" => {
            want(0)?;
            Instr::EndIf
        }
        "LOOP" => {
            want(1)?;
            Instr::Loop {
                n: int(&args[0]).ok_or_else(|| bad(&args[0]))?,
            }
        }
        "ENDLOOP" => {
            want(0)?;
            Instr::EndLoop
        }
        "NOP" => {
            want(0)?;
            Instr::Nop
        }
        "EDGE" => {
            want(2)?;
            Instr::Edge {
                d: r(0)?,
                addr: m(1)?,
            }
        }

        "DOT" | "LERP" => {
            want(3)?;
            let (d, a, b) = (r(0)?, r(1)?, r(2)?);
            if op == "DOT" {
                Instr::Dot { d, a, b }
            } else {
                Instr::Lerp { d, a, b }
            }
        }
        "MMA" => {
            want(4)?;
            Instr::Mma {
                d: r(0)?,
                a: r(1)?,
                b: r(2)?,
                c: r(3)?,
            }
        }
        "RCP" => {
            want(2)?;
            Instr::Rcp { d: r(0)?, a: s(1)? }
        }
        "LD" | "LDO" | "LDS" | "LDL" => {
            want(2)?;
            let (d, addr) = (r(0)?, m(1)?);
            match op.as_str() {
                "LD" => Instr::Ld { d, addr },
                "LDO" => Instr::Ldo { d, addr },
                "LDL" => Instr::Ldl { d, addr },
                _ => Instr::Lds { d, addr },
            }
        }
        "ST" | "STS" | "STL" => {
            want(2)?;
            let (addr, v) = (m(0)?, s(1)?);
            match op.as_str() {
                "ST" => Instr::St { addr, v },
                "STL" => Instr::Stl { addr, v },
                _ => Instr::Sts { addr, v },
            }
        }
        "DMA" => {
            want(3)?;
            Instr::Dma {
                dst: s(0)?,
                src: s(1)?,
                n: s(2)?,
            }
        }
        "SEND" => {
            want(3)?;
            Instr::Send {
                peer: s(0)?,
                buf: s(1)?,
                n: s(2)?,
            }
        }
        "RECV" => {
            want(3)?;
            Instr::Recv {
                peer: s(0)?,
                buf: s(1)?,
                n: s(2)?,
            }
        }
        "BAR" => {
            want(0)?;
            Instr::Bar
        }
        "GSYNC" => {
            want(0)?;
            Instr::Gsync
        }
        "FENCE" => {
            want(0)?;
            Instr::Fence
        }
        "ATOM" | "ATOMS" => {
            want(3)?;
            Instr::Atom {
                d: r(0)?,
                addr: m(1)?,
                v: s(2)?,
                shared: op == "ATOMS",
            }
        }
        "PIX" => {
            want(1)?;
            Instr::Pix { v: s(0)? }
        }
        "SHFL" => {
            want(3)?;
            Instr::Shfl {
                d: r(0)?,
                a: s(1)?,
                lane: s(2)?,
            }
        }
        "VOTE.ANY" | "VOTE.ALL" => {
            want(2)?;
            Instr::Vote {
                all: op.ends_with("ALL"),
                p: p(0)?,
                q: p(1)?,
            }
        }
        _ => return Err(AsmErrorKind::UnknownOp(op)),
    };
    Ok(Some(ins))
}

/// Assembles source into instructions without block validation.
pub fn parse(source: &str) -> Result<(Vec<Instr>, Vec<usize>), AsmError> {
    let mut code = Vec::new();
    let mut lines = Vec::new();
    for (line, raw) in source.lines().enumerate() {
        let text = raw.split([';', '#']).next().unwrap_or("");
        if text.trim_start().starts_with('.') {
            continue;
        }
        match parse_line(text) {
            Ok(Some(i)) => {
                code.push(i);
                lines.push(line);
            }
            Ok(None) => {}
            Err(kind) => return Err(AsmError { line, kind }),
        }
    }
    Ok((code, lines))
}

/// Assembles and validates block structure.
pub fn assemble(source: &str) -> Result<Program, AsmError> {
    let (code, lines) = parse(source)?;
    let mut prog = Program::new(code).map_err(|e| {
        let line = e.line().and_then(|i| lines.get(i).copied()).unwrap_or(0);
        AsmError {
            line,
            kind: AsmErrorKind::Shape(e),
        }
    })?;
    for (line, raw) in source.lines().enumerate() {
        let text = raw.split([';', '#']).next().unwrap_or("").trim();
        let Some(rest) = text.strip_prefix('.') else {
            continue;
        };
        let (name, arg) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        let bad = |a: &str| AsmError {
            line,
            kind: AsmErrorKind::BadOperand(a.to_string()),
        };
        if name.eq_ignore_ascii_case("smem") {
            let n = arg
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|n| (1..=8192).contains(n))
                .ok_or_else(|| bad(arg.trim()))?;
            prog.smem = Some(n);
            continue;
        }
        if name.eq_ignore_ascii_case("block") {
            let n = arg
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|n| (32..=512).contains(n) && n % 32 == 0)
                .ok_or_else(|| bad(arg.trim()))?;
            prog.block = Some(n);
            continue;
        }
        if !name.eq_ignore_ascii_case("warps") {
            return Err(AsmError {
                line,
                kind: AsmErrorKind::UnknownOp(format!(".{}", name.to_ascii_uppercase())),
            });
        }
        let n = arg
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|n| (1..=16).contains(n))
            .ok_or_else(|| bad(arg.trim()))?;
        prog.warps = Some(n);
    }
    Ok(prog)
}
