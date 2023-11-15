use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::ast;
use crate::ir::Stmt::SetHeap;
use crate::ir::{Const, Function, Instr, Prop, Stmt, TypeRepr};
use crate::run::{self, Book, Def, Ptr, Val};

pub fn compile_term(book: &Book, fid: Val) -> Function {
  let mut lowering = Lowering {
    newx: Rc::new(Cell::new(0)),
    book,
    target: "x".to_string(),
    vars: Rc::new(RefCell::new(HashMap::new())),
    stmts: vec![],
  };

  lowering.call(fid);
  lowering.stmts.push(Stmt::Return(Instr::True));

  Function {
    name: ast::val_to_name(fid),
    body: lowering.stmts,
  }
}

fn assert_is_atom(ptr: Ptr) -> Instr {
  if ptr.is_ref() {
    Instr::NewPtr {
      tag: Instr::from(Const::REF).into(),
      value: Instr::from(Const::F(ast::val_to_name(ptr.val()))).into(),
    }
  } else {
    Instr::NewPtr {
      tag: Instr::from(compile_tag(ptr.tag())).into(),
      value: Instr::Hex(ptr.val()).into(),
    }
  }
}

fn compile_tag(tag: run::Tag) -> Const {
  match tag {
    run::VR1 => Const::VR1,
    run::VR2 => Const::VR2,
    run::RD1 => Const::RD1,
    run::RD2 => Const::RD2,
    run::REF => Const::REF,
    run::ERA => Const::ERA,
    run::NUM => Const::NUM,
    run::OP2 => Const::OP2,
    run::OP1 => Const::OP1,
    run::MAT => Const::MAT,
    run::CT0 => Const::CT0,
    run::CT1 => Const::CT1,
    run::CT2 => Const::CT2,
    run::CT3 => Const::CT3,
    run::CT4 => Const::CT4,
    run::CT5 => Const::CT5,
    _ => unreachable!(),
  }
}

// TODO: HVM-Lang must always output in this form.
fn adjust_redex(rf: Ptr, rx: Ptr) -> (Ptr, Ptr) {
  if rf.is_skp() && !rx.is_skp() {
    return (rf, rx);
  } else if !rf.is_skp() && rx.is_skp() {
    return (rx, rf);
  } else {
    println!("Invalid redex. Compiled HVM requires that ALL defs are in the form:");
    println!("@name = ROOT");
    println!("  & ATOM ~ TERM");
    println!("  & ATOM ~ TERM");
    println!("  & ATOM ~ TERM");
    println!("  ...");
    println!("Where ATOM must be either a ref (`@foo`), a num (`#123`), or an era (`*`).");
    println!("If you used HVM-Lang, please report on https://github.com/HigherOrderCO/hvm-lang.");
    panic!("Invalid HVMC file.");
  }
}

type Target = String;

#[derive(Clone)]
struct Lowering<'book> {
  newx: Rc<Cell<usize>>,
  book: &'book Book,
  vars: Rc<RefCell<HashMap<Ptr, String>>>,
  target: Target,
  stmts: Vec<Stmt>,
}

impl Lowering<'_> {
  /// returns a fresh variable: 'v<NUM>'
  fn fresh(&mut self) -> Instr {
    let name = format!("v{}", self.newx.get());
    self.newx.set(self.newx.get() + 1);
    Instr::Prop(Prop::Var(name))
  }

  /// returns a fresh variable: 'v<NUM>'
  fn fresh_name(&mut self) -> String {
    let name = format!("v{}", self.newx.get());
    self.newx.set(self.newx.get() + 1);
    name
  }

  fn call(&mut self, fid: Val) {
    let def = &self.book.defs[fid as usize];
    self.burn(def, def.node[0].1, self.target.clone());
    for (rf, rx) in &def.rdex {
      let (rf, rx) = adjust_redex(*rf, *rx);
      let ref_name = self.fresh_name();
      self.stmts.push(Stmt::Let {
        name: ref_name.clone(),
        value: assert_is_atom(rf),
      });
      self.burn(def, rx, ref_name);
    }
  }

  fn declare_fresh(&mut self, type_repr: TypeRepr) -> String {
    let name = self.fresh_name();
    self.stmts.push(Stmt::Val {
      name: name.clone(),
      type_repr,
    });
    name
  }

  fn declare(&mut self, name: String, type_repr: TypeRepr) -> String {
    self.stmts.push(Stmt::Val {
      name: name.clone(),
      type_repr,
    });
    name
  }

  fn define_fresh(&mut self, value: Instr) -> String {
    let name = self.fresh_name();
    self.stmts.push(Stmt::Let {
      name: name.clone(),
      value,
    });
    name
  }

  fn define(&mut self, name: String, value: Instr) -> String {
    self.stmts.push(Stmt::Let {
      name: name.clone(),
      value,
    });
    name
  }

  fn assign(&mut self, prop: Prop, value: Instr) {
    self.stmts.push(Stmt::Assign { name: prop, value });
  }

  /// Fork returning Self
  fn fork(&self) -> Self {
    let mut new_fork = self.clone();
    new_fork.stmts = vec![];
    new_fork
  }

  /// Fork returning vec of statements
  fn fork_on(&self, f: impl FnOnce(&mut Self)) -> Vec<Stmt> {
    let mut fork = self.fork();
    f(&mut fork);
    fork.stmts
  }

  /// Generates code
  fn make(&mut self, def: &Def, ptr: Ptr, target: Target) {
    todo!()
  }

  /// @loop = (?<(#0 (x y)) R> R) & @loop ~ (x y)
  fn burn(&mut self, def: &Def, ptr: Ptr, target: Target) {
    // (<?(ifz ifs) ret> ret) ~ (#X R)
    // ------------------------------- fast match
    // if X == 0:
    //   ifz ~ R
    //   ifs ~ *
    // else:
    //   ifz ~ *
    //   ifs ~ (#(X-1) R)
    // When ifs is REF, tail-call optimization is applied.
    if ptr.tag() == run::CT0 {
      let (mat, rty) = def.node[ptr.val() as usize];
      if mat.tag() == run::MAT {
        let got @ (cse, rtx) = def.node[rty.val() as usize];
        let rtz = if rty.tag() == run::VR1 { got.0 } else { got.1 };
        if cse.tag() == run::CT0 && rtx.is_var() && rtx == rtz {
          let (ifz, ifs) = def.node[cse.val() as usize];
          let c_z = self.declare_fresh(TypeRepr::HvmPtr);
          let c_s = self.declare_fresh(TypeRepr::HvmPtr);
          // FAST MATCH
          // if tag(target) = CT0 && is-num(get-heap(val(target))
          self.stmts.push(Stmt::Instr(Instr::If {
            cond: Instr::from(target.clone())
              .eq(Instr::from(Const::CT0))
              .and(
                Instr::GetHeap {
                  idx: Instr::from(target.clone()).into(),
                  port: Instr::from(Const::P1).into(),
                }
                .is_num(),
              )
              .into(),
            then: self.fork_on(|lowering| {
              // self.anni += 2
              lowering.assign(Prop::Anni, Instr::from(Prop::Anni).add(Instr::Int(2)));

              // self.oper += 1
              lowering.assign(Prop::Oper, Instr::from(Prop::Anni).add(Instr::Int(1)));

              // let num = self.heap.get(target.val(), P1)
              let num = lowering.define(format!("{}_x", target), Instr::GetHeap {
                idx: Instr::from(target.clone()).val().into(),
                port: Instr::from(Const::P1).into(),
              });

              // let res = self.heap.get(target.val(), P2)
              let res = lowering.define(format!("{}_y", target), Instr::GetHeap {
                idx: Instr::from(target.clone()).val().into(),
                port: Instr::from(Const::P2).into(),
              });

              // if num.val() == 0
              //   self.free(target.val())
              //   c_z = res
              //   c_s = ERAS
              // else
              //   self.heap.set(target.val(), P1, Ptr::new(NUM, num.val() - 1))
              //   c_z = ERAS
              //   c_s = target
              lowering.stmts.push(Stmt::Instr(Instr::If {
                cond: Instr::from(num.clone()).eq(Instr::Int(0)).into(),
                then: lowering.fork_on(|lowering| {
                  lowering
                    .stmts
                    .push(Stmt::Free(Instr::from(target.clone()).val()));
                  lowering.assign(Prop::Var(c_z.clone()), Instr::from(res.clone()));
                  lowering.assign(Prop::Var(c_s.clone()), Instr::from(Const::ERAS));
                }),
                otherwise: lowering.fork_on(|lowering| {
                  lowering.stmts.push(Stmt::SetHeap {
                    idx: Instr::from(target.clone()).val().into(),
                    port: Instr::from(Const::P1).into(),
                    value: Instr::new_ptr(
                      Instr::from(Const::NUM),
                      Instr::from(num).sub(Instr::Int(1)),
                    )
                    .into(),
                  });
                  lowering.assign(Prop::Var(c_z.clone()), Instr::from(Const::ERAS));
                  lowering.assign(Prop::Var(c_s.clone()), Instr::from(target.clone()));
                }),
              }))
            }),
            otherwise: self.fork_on(|lowering| {
              let lam = lowering.define_fresh(Instr::Alloc { size: 1 });
              let mat = lowering.define_fresh(Instr::Alloc { size: 1 });
              let cse = lowering.define_fresh(Instr::Alloc { size: 1 });
              lowering.stmts.push(SetHeap {
                // self.heap.set(lam, P1, Ptr::new(MAT, mat));
                idx: Instr::from(lam.clone()).into(),
                port: Instr::from(Const::P1).into(),
                value: Instr::new_ptr(Const::MAT, Instr::from(mat.clone())).into(),
              });
              lowering.stmts.push(SetHeap {
                // self.heap.set(lam, P2, Ptr::new(VR2, mat));
                idx: Instr::from(lam.clone()).into(),
                port: Instr::from(Const::P2).into(),
                value: Instr::new_ptr(Const::VR2, Instr::from(mat.clone())).into(),
              });
              lowering.stmts.push(SetHeap {
                // self.heap.set(mat, P1, Ptr::new(CT0, cse));
                idx: Instr::from(mat.clone()),
                port: Instr::from(Const::P1),
                value: Instr::new_ptr(Const::CT0, Instr::from(cse.clone())),
              });
              lowering.stmts.push(SetHeap {
                // self.heap.set(mat, P2, Ptr::new(VR2, cse));
                idx: Instr::from(mat.clone()),
                port: Instr::from(Const::P2),
                value: Instr::new_ptr(Const::VR2, Instr::from(lam.clone())),
              });
              lowering.stmts.push(
                Instr::new_ptr(Const::CT0, Instr::from(lam.clone()))
                  .link(Instr::from(target.clone())),
              );
              lowering.assign(
                Prop::Var(c_z.clone()),
                Instr::new_ptr(Const::VR1, Instr::from(cse.clone())),
              );
              lowering.assign(
                Prop::Var(c_s.clone()),
                Instr::new_ptr(Const::VR2, Instr::from(cse.clone())),
              );
            }),
          }));
          self.burn(def, ifz, c_z.clone());
          self.burn(def, ifs, c_s.clone());
          return;
        }
      }
    }

    // {p1 p2} <~ #N
    // ------------- fast copy
    // p1 <~ #N
    // p2 <~ #N
    if ptr.is_ctr() && ptr.tag() > run::CT0 {
      let x1 = self.declare_fresh(TypeRepr::HvmPtr);
      let x2 = self.declare_fresh(TypeRepr::HvmPtr);
      let (p1, p2) = def.node[ptr.val() as usize];

      // FAST COPY
      // if tag(target) = NUM
      //   self.comm += 1
      //   x1 = target
      //   x2 = target
      // else
      //   let lc = self.alloc(1)
      //   x1 = Ptr::new(VR1, lc)
      //   x2 = Ptr::new(VR2, lc)
      //   self.link(Ptr::new(ptr.tag(), lc), target)
      self.stmts.push(Stmt::Instr(Instr::If {
        cond: Instr::from(target.clone())
          .tag()
          .eq(Instr::from(Const::NUM))
          .into(),
        then: self.fork_on(|lowering| {
          lowering.assign(Prop::Comm, Instr::from(Prop::Comm).add(Instr::Int(1)));
          lowering.assign(Prop::Var(x1.clone()), Instr::from(target.clone()));
          lowering.assign(Prop::Var(x2.clone()), Instr::from(target.clone()));
        }),
        otherwise: self.fork_on(|lowering| {
          let lc = lowering.define_fresh(Instr::Alloc { size: 1 });
          lowering.assign(Prop::Var(x1.clone()), Instr::new_ptr(Const::VR1, lc.clone().into()));
          lowering.assign(Prop::Var(x2.clone()), Instr::new_ptr(Const::VR2, lc.clone().into()));
          lowering.stmts.push(
            Instr::new_ptr(compile_tag(ptr.tag()), lc.into()).link(Instr::from(target.clone())),
          );
        }),
      }));

      self.burn(def, p1, x1.clone());
      self.burn(def, p2, x2.clone());
      return;
    }

    // (p1 p2) <~ (x1 x2)
    // ------------------ fast apply
    // p1 <~ x1
    // p2 <~ x2
    if ptr.is_ctr() && ptr.tag() == run::CT0 {
      let (p1, p2) = def.node[ptr.val() as usize];

      let x1 = self.declare_fresh(TypeRepr::HvmPtr);
      let x2 = self.declare_fresh(TypeRepr::HvmPtr);

      // FAST APPLY
      // if tag(target) = ptr.tag()
      //   self.anni += 1
      //   x1 = heap-get(val(target), P1)
      //   x2 = heap-get(val(target), P2)
      // else
      //   let lc = self.alloc(1)
      //   x1 = Ptr::new(VR1, lc)
      //   x2 = Ptr::new(VR2, lc)
      //   self.link(Ptr::new(ptr.tag(), lc), target)
      self.stmts.push(Stmt::Instr(Instr::If {
        cond: Instr::from(target.clone())
          .tag()
          .eq(Instr::from(compile_tag(ptr.tag())))
          .into(),
        then: self.fork_on(|lowering| {
          lowering.assign(Prop::Anni, Instr::from(Prop::Anni).add(Instr::Int(1)));
          lowering.assign(Prop::Var(x1.clone()), Instr::GetHeap {
            idx: Instr::from(target.clone()).into(),
            port: Instr::from(Const::P1).into(),
          });
          lowering.assign(Prop::Var(x2.clone()), Instr::GetHeap {
            idx: Instr::from(target.clone()).into(),
            port: Instr::from(Const::P2).into(),
          });
          lowering
            .stmts
            .push(Stmt::Free(Instr::from(target.clone()).val().into()))
        }),
        otherwise: self.fork_on(|lowering| {
          let lc = lowering.define_fresh(Instr::Alloc { size: 1 });
          lowering.assign(Prop::Var(x1.clone()), Instr::new_ptr(Const::VR1, lc.clone().into()));
          lowering.assign(Prop::Var(x2.clone()), Instr::new_ptr(Const::VR2, lc.clone().into()));
          lowering.stmts.push(
            Instr::new_ptr(compile_tag(ptr.tag()), lc.into()).link(Instr::from(target.clone())),
          );
        }),
      }));

      self.burn(def, p1, x1.clone());
      self.burn(def, p2, x2.clone());
      return;
    }

    // ATOM <~ *
    // --------- fast erase
    // nothing
    if ptr.is_num() || ptr.is_era() {
      // FAST ERASE
      self.stmts.push(Stmt::Instr(Instr::If {
        cond: Instr::from(target.clone()).is_skp().into(),
        then: self.fork_on(|lowering| {
          lowering.assign(Prop::Eras, Instr::from(Prop::Eras).add(Instr::Int(1)));
        }),
        otherwise: self.fork_on(|lowering| {
          lowering.make(def, ptr, target.clone());
        }),
      }));
      return;
    }
  }
}
