//! Regression test for a translator panic on an `if` whose block parameter lives in a register
//! while the `then` block reassigns a local via `local.tee`/`local.set`.
//!
//! The `if` saves its block parameter (the negated `f64` below) in the `f64` register. The `then`
//! block's `local.tee`/`local.set` then links that same register to the local. When the `end`
//! (or `else`) restores the saved `else` operands, the translator re-allocated the register
//! without first discarding the stale local link, tripping the debug-assert
//! `"a register operand already exists on the stack"` in `OperandStack::alloc_reg`
//! (`engine/translator/func/stack/operands.rs`). Found by a differential fuzz run against Wasmtime.

use wasmi::{Engine, Instance, Module, Store};

fn instantiate(wat: &str) -> (Store<()>, Instance) {
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());
    let module = Module::new(&engine, wat).expect("module compiles");
    let instance = Instance::new(&mut store, &module, &[]).expect("instantiates");
    (store, instance)
}

/// An `if` with a block parameter in a register and a `local.tee` of that local in the `then`
/// block, with no `else` (the `end` restores the `else` operands).
#[test]
fn if_tee_block_param_in_register_without_else() {
    let wat = r#"
        (module
            (func (export "f") (param f64 i32) (result f64)
                local.get 0
                f64.neg
                local.get 1
                (if (param f64) (result f64)
                    (then local.tee 0)
                )
            )
        )
    "#;
    let (mut store, instance) = instantiate(wat);
    let f = instance
        .get_typed_func::<(f64, i32), f64>(&store, "f")
        .expect("func f");
    // When the condition is false the block parameter (-x) passes straight through.
    assert_eq!(f.call(&mut store, (4.0, 0)).expect("call"), -4.0);
    // When the condition is true `local.tee` writes -x into local 0 and leaves it as the result.
    assert_eq!(f.call(&mut store, (4.0, 1)).expect("call"), -4.0);
}

/// The same shape with an explicit (empty) `else`, which reaches the panic through `push_else`.
#[test]
fn if_tee_block_param_in_register_with_else() {
    let wat = r#"
        (module
            (func (export "f") (param f64 i32) (result f64)
                local.get 0
                f64.neg
                local.get 1
                (if (param f64) (result f64)
                    (then local.tee 0)
                    (else)
                )
            )
        )
    "#;
    let (mut store, instance) = instantiate(wat);
    let f = instance
        .get_typed_func::<(f64, i32), f64>(&store, "f")
        .expect("func f");
    assert_eq!(f.call(&mut store, (4.0, 0)).expect("call"), -4.0);
    assert_eq!(f.call(&mut store, (4.0, 1)).expect("call"), -4.0);
}

/// `local.set` + `local.get` of an integer local inside the `then` block (the `i32` register).
#[test]
fn if_set_block_param_in_register() {
    let wat = r#"
        (module
            (func (export "f") (param i32 i32) (result i32)
                local.get 0
                i32.eqz
                local.get 1
                (if (param i32) (result i32)
                    (then local.set 0 local.get 0)
                )
            )
        )
    "#;
    let (mut store, instance) = instantiate(wat);
    let f = instance
        .get_typed_func::<(i32, i32), i32>(&store, "f")
        .expect("func f");
    // condition false: (eqz 5) == 0 passes through.
    assert_eq!(f.call(&mut store, (5, 0)).expect("call"), 0);
    // condition true: local 0 is set to (eqz 5)==0, then read back.
    assert_eq!(f.call(&mut store, (5, 1)).expect("call"), 0);
    assert_eq!(f.call(&mut store, (0, 1)).expect("call"), 1);
}
