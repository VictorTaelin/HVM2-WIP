// Despite the file name, this is not actually a JIT (yet).

use crate::{
  host::Host,
  run::{DefType, Instruction, Port, Tag},
};
use std::{
  fmt::{self, Write},
  hash::{DefaultHasher, Hasher},
};

pub fn compile_book(host: &Host) -> String {
  _compile_book(host).unwrap()
}

fn _compile_book(host: &Host) -> Result<String, fmt::Error> {
  let mut code = Code::default();

  writeln!(code, "#![allow(non_upper_case_globals, unused_imports)]")?;
  writeln!(code, "use crate::{{host::{{Host, DefRef}}, run::*, ops::Op::*}};")?;
  writeln!(code, "use std::borrow::Cow;")?;
  writeln!(code, "")?;

  writeln!(code, "pub fn host() -> Host {{")?;
  code.indent(|code| {
    writeln!(code, "let mut host = Host::default();")?;
    for raw_name in host.defs.keys() {
      let name = sanitize_name(raw_name);
      writeln!(code, r##"host.insert(r#"{raw_name}"#, DefRef::Static(&DEF_{name}));"##)?;
    }
    writeln!(code, "host")
  })?;
  writeln!(code, "}}\n")?;

  for (raw_name, def) in &host.defs {
    let name = sanitize_name(raw_name);
    write!(
      code,
      "pub static DEF_{name}: Def = Def {{ labs: LabSet {{ min_safe: {}, bits: Cow::Borrowed(&[",
      def.labs.min_safe()
    )?;
    for (i, word) in def.labs.bits.iter().enumerate() {
      if i != 0 {
        write!(code, ", ")?;
      }
      write!(code, "0x{:x}", word)?;
    }
    writeln!(code, "]) }}, inner: DefType::Native(call_{name}) }};")?;
  }

  writeln!(code)?;

  for (raw_name, def) in &host.defs {
    compile_def(&mut code, host, raw_name, match &def.inner {
      DefType::Net(n) => &n.instr,
      DefType::Native(_) => unreachable!(),
    })?;
  }

  Ok(code.code)
}

fn compile_def(code: &mut Code, host: &Host, raw_name: &str, instr: &[Instruction]) -> fmt::Result {
  let name = sanitize_name(raw_name);
  writeln!(code, "pub fn call_{name}(net: &mut Net, to: Port) {{")?;
  code.indent(|code| {
    code.write_str("let t0 = Trg::port(to);\n")?;
    for instr in instr {
      match instr {
        Instruction::Const { trg, port } => {
          writeln!(code, "let {trg} = Trg::port({});", print_port(host, port))
        }
        Instruction::Link { a, b } => writeln!(code, "net.link_trg({a}, {b});"),
        Instruction::Set { trg, port } => {
          writeln!(code, "net.link_trg({trg}, Trg::port({}));", print_port(host, port))
        }
        Instruction::Ctr { lab, trg, lft, rgt } => writeln!(code, "let ({lft}, {rgt}) = net.do_ctr({lab}, {trg});"),
        Instruction::Op2 { op, trg, lft, rgt } => writeln!(code, "let ({lft}, {rgt}) = net.do_op2({op:?}, {trg});"),
        Instruction::Op1 { op, num, trg, rgt } => writeln!(code, "let {rgt} = net.do_op1({op:?}, {num}, {trg});"),
        Instruction::Mat { trg, lft, rgt } => writeln!(code, "let ({lft}, {rgt}) = net.do_mat({trg});"),
        Instruction::Wires { av, aw, bv, bw } => writeln!(code, "let ({av}, {aw}, {bv}, {bw}) = net.do_wires();"),
      }?;
    }
    Ok(())
  })?;
  writeln!(code, "}}")?;
  code.write_char('\n')?;

  Ok(())
}

fn print_port(host: &Host, port: &Port) -> String {
  if port == &Port::ERA {
    "Port::ERA".to_owned()
  } else if port.tag() == Tag::Ref {
    let name = sanitize_name(&host.back[&port.loc()]);
    format!("Port::new_ref(&DEF_{name})")
  } else if port.tag() == Tag::Num {
    format!("Port::new_num({})", port.num())
  } else {
    unreachable!()
  }
}

#[derive(Default)]
struct Code {
  code: String,
  indent: usize,
  on_newline: bool,
}

impl Code {
  fn indent<T>(&mut self, cb: impl FnOnce(&mut Code) -> T) -> T {
    self.indent += 1;
    let val = cb(self);
    self.indent -= 1;
    val
  }
}

impl Write for Code {
  fn write_str(&mut self, s: &str) -> fmt::Result {
    for s in s.split_inclusive('\n') {
      if self.on_newline {
        for _ in 0 .. self.indent {
          self.code.write_str("  ")?;
        }
      }

      self.on_newline = s.ends_with('\n');
      self.code.write_str(s)?;
    }

    Ok(())
  }
}

fn sanitize_name(name: &str) -> String {
  if !name.contains('.') {
    name.to_owned()
  } else {
    let mut hasher = DefaultHasher::new();
    hasher.write(name.as_bytes());
    let hash = hasher.finish();
    let mut sanitized = name.replace('.', "_");
    sanitized.push_str("__");
    write!(sanitized, "__{:016x}", hash).unwrap();
    sanitized
  }
}
