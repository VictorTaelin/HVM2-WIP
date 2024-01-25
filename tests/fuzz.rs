#![cfg(feature = "_fuzz")]

use hvmc::{
  fuzz::*,
  run::{Loc, Net, Port, Tag},
  trace,
};

use serial_test::serial;

#[test]
#[serial]
fn fuzz_var_link_link_var() {
  assert!(cfg!(not(feature = "_fuzz_no_free")));
  trace::set_hook();
  let mut bad_link = 0;
  let mut bad_free = 0;
  Fuzzer::default().fuzz(|fuzz| {
    unsafe { trace::_reset_traces() };
    let heap = Net::init_heap(256);
    let mut net = Net::new(&heap);
    let x = net.alloc();
    let y = net.alloc();
    let z = net.alloc();
    let a = Port::new_var(x.clone());
    let b = Port::new_var(x.other_half());
    let c = Port::new_var(y.clone());
    let d = Port::new_var(y.other_half());
    let e = Port::new_var(z.clone());
    let f = Port::new_var(z.other_half());
    net.link_port_port(a.clone(), b.clone());
    net.link_port_port(c.clone(), d.clone());
    net.link_port_port(e.clone(), f.clone());
    let mut n0 = net.fork(0, 2);
    let mut n1 = net.fork(1, 2);
    fuzz.scope(|s| {
      s.spawn(|| {
        let (x, y) = fuzz.maybe_swap(b.clone(), c.clone());
        n0.link_wire_wire(x.wire(), y.wire());
      });
      s.spawn(|| {
        let (x, y) = fuzz.maybe_swap(d.clone(), e.clone());
        n1.link_wire_wire(x.wire(), y.wire());
      });
    });
    for (x, y) in [(a.clone(), f.clone()), (f, a)] {
      let mut w = x.wire();
      loop {
        let p = Port(w.loc().val().read());
        if p == y {
          break;
        }
        if p.tag() == Tag::Red {
          w = p.wire();
          continue;
        }
        bad_link += 1;
        return;
      }
    }
    for x in [b, c, d, e] {
      if x.loc().val().read() != Port::FREE.0 {
        bad_free += 1;
      }
    }
  });
  dbg!(bad_link, bad_free);
}

#[test]
#[serial]
fn fuzz_pri_link_link_pri() {
  assert!(cfg!(not(feature = "_fuzz_no_free")));
  trace::set_hook();
  Fuzzer::default().fuzz(|fuzz| {
    unsafe { trace::_reset_traces() };
    let p = Port::new(Tag::Ctr, 0, Loc::NULL);
    let q = Port::new(Tag::Ctr, 1, Loc::NULL);
    let heap = Net::init_heap(256);
    let mut net = Net::new(&heap);
    let x = net.alloc();
    let a = Port::new_var(x.clone());
    let b = Port::new_var(x.other_half());
    net.link_port_port(a.clone(), b.clone());
    let mut n0 = net.fork(0, 2);
    let mut n1 = net.fork(1, 2);
    fuzz.scope(|s| {
      s.spawn(|| {
        n0.link_wire_port(a.wire(), p);
      });
      s.spawn(|| {
        n1.link_wire_port(b.wire(), q);
      });
    });
    assert!(n0.rdex.len() == 1 || n1.rdex.len() == 1);
    for x in [a, b] {
      assert_eq!(x.loc().val().read(), Port::FREE.0);
    }
  })
}

#[test]
#[serial]
fn fuzz_var_link_link_pri() {
  assert!(cfg!(not(feature = "_fuzz_no_free")));
  trace::set_hook();
  Fuzzer::default().fuzz(|fuzz| {
    unsafe { trace::_reset_traces() };
    let heap = Net::init_heap(256);
    let mut net = Net::new(&heap);
    let x = net.alloc();
    let y = net.alloc();
    let a = Port::new_var(x.clone());
    let b = Port::new_var(x.other_half());
    let c = Port::new_var(y.clone());
    let d = Port::new_var(y.other_half());
    net.link_port_port(a.clone(), b.clone());
    net.link_port_port(c.clone(), d.clone());
    let mut n0 = net.fork(0, 2);
    let mut n1 = net.fork(1, 2);
    fuzz.scope(|s| {
      s.spawn(|| {
        let (x, y) = fuzz.maybe_swap(b.clone(), c.clone());
        n0.link_wire_wire(x.wire(), y.wire());
      });
      s.spawn(|| {
        n1.link_wire_port(d.wire(), Port::ERA);
      });
    });
    let at = Port(a.loc().val().read());
    assert_eq!(at, Port::ERA);
    for x in [b, c, d] {
      assert_eq!(Port(x.loc().val().read()), Port::FREE);
    }
  })
}

#[test]
#[serial]
#[ignore = "very slow"] // takes ~50m on my M3 Max (or ~13.5h with tracing enabled)
fn fuzz_var_link_link_link_var() {
  assert!(cfg!(feature = "_fuzz_no_free"));
  trace::set_hook();
  let heap = Net::init_heap(256);
  Fuzzer::default().fuzz(|fuzz| {
    unsafe { trace::_reset_traces() };
    let mut net = Net::new(&heap);
    let x = net.alloc();
    let y = net.alloc();
    let z = net.alloc();
    let w = net.alloc();
    let a = Port::new_var(x.clone());
    let b = Port::new_var(x.other_half());
    let c = Port::new_var(y.clone());
    let d = Port::new_var(y.other_half());
    let e = Port::new_var(z.clone());
    let f = Port::new_var(z.other_half());
    let g = Port::new_var(w.clone());
    let h = Port::new_var(w.other_half());
    net.link_port_port(a.clone(), b.clone());
    net.link_port_port(c.clone(), d.clone());
    net.link_port_port(e.clone(), f.clone());
    net.link_port_port(g.clone(), h.clone());
    let mut n0 = net.fork(0, 3);
    let mut n1 = net.fork(1, 3);
    let mut n2 = net.fork(2, 3);
    fuzz.scope(|s| {
      s.spawn(|| {
        let (x, y) = fuzz.maybe_swap(b.clone(), c.clone());
        n0.link_wire_wire(x.wire(), y.wire());
      });
      s.spawn(|| {
        let (x, y) = fuzz.maybe_swap(d, e);
        n1.link_wire_wire(x.wire(), y.wire());
      });
      s.spawn(|| {
        let (x, y) = fuzz.maybe_swap(f, g);
        n2.link_wire_wire(x.wire(), y.wire());
      });
    });
    let at = Port(a.loc().val().read());
    let ht = Port(h.loc().val().read());
    if at != h || ht != a {
      dbg!(&a, &h, &at, &ht);
      panic!("invalid link")
    }
  })
}
