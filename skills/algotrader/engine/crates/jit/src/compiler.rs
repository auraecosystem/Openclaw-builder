//! Cranelift IR generation for fused filter conditions.
//!
//! Translates a slice of `JitCondition` into a native function that evaluates
//! all conditions branchlessly (using `band`) and writes the result mask.
//! Thresholds are passed as runtime f32 arguments so CMA-ES can vary them
//! without recompilation. Indicator data pointers are also runtime arguments,
//! resolved per-call from the DataStore.

use cranelift::prelude::*;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};

use engine_data::Indicator;

/// Comparison operator for JIT filter conditions.
///
/// Mirrors `engine_pipeline::blocks::CmpOp` but defined here to avoid a
/// cyclic dependency between engine-jit and engine-pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    Gt,
    Ge,
    Lt,
    Le,
}

/// A single indicator-threshold condition for JIT compilation.
///
/// Mirrors `engine_pipeline::fuse::FusedCondition` but lives in the JIT crate
/// to break the dependency cycle.
#[derive(Clone, Debug)]
pub struct JitCondition {
    pub indicator: Indicator,
    pub op: CmpOp,
    pub threshold: f32,
}

/// A compiled native filter function.
///
/// Holds the `JITModule` to keep the code alive. The function pointer is valid
/// for the lifetime of this struct.
pub struct JitFilter {
    /// Number of conditions baked into this function.
    pub n_conditions: usize,
    /// The raw function pointer. Signature:
    ///   fn(data_ptrs: [*const f32; N], input: *const u8, output: *mut u8,
    ///      len: i64, thresholds: [f32; N])
    /// Encoded as a flat ABI: N pointer args, then input/output/len, then N f32s.
    fn_ptr: *const u8,
    /// Kept alive so the JIT code isn't freed.
    _module: JITModule,
}

// JitFilter is Send+Sync because the compiled code is immutable after finalization
// and the function pointer is safe to call from any thread.
unsafe impl Send for JitFilter {}
unsafe impl Sync for JitFilter {}

impl JitFilter {
    /// Execute the JIT-compiled filter.
    ///
    /// # Safety
    /// - `data_ptrs` must have exactly `self.n_conditions` valid pointers, each
    ///   pointing to at least `len` f32 values.
    /// - `thresholds` must have exactly `self.n_conditions` values.
    /// - `input` must point to at least `len` u8 values.
    /// - `output` must point to at least `len` u8 values (will be written).
    pub unsafe fn call_raw(
        &self,
        data_ptrs: &[*const f32],
        input: *const u8,
        output: *mut u8,
        len: i64,
        thresholds: &[f32],
    ) {
        assert_eq!(data_ptrs.len(), self.n_conditions);
        assert_eq!(thresholds.len(), self.n_conditions);

        match self.n_conditions {
            1 => self.call_1(data_ptrs, input, output, len, thresholds),
            2 => self.call_2(data_ptrs, input, output, len, thresholds),
            3 => self.call_3(data_ptrs, input, output, len, thresholds),
            4 => self.call_4(data_ptrs, input, output, len, thresholds),
            5 => self.call_5(data_ptrs, input, output, len, thresholds),
            6 => self.call_6(data_ptrs, input, output, len, thresholds),
            7 => self.call_7(data_ptrs, input, output, len, thresholds),
            8 => self.call_8(data_ptrs, input, output, len, thresholds),
            9 => self.call_9(data_ptrs, input, output, len, thresholds),
            10 => self.call_10(data_ptrs, input, output, len, thresholds),
            _ => panic!("JIT filter supports up to 10 conditions, got {}", self.n_conditions),
        }
    }
}

// Generate typed call wrappers via macro to avoid unsafe transmute gymnastics.
macro_rules! jit_call_impl {
    ($name:ident, $n:expr, ($($di:tt),*), ($($ti:tt),*)) => {
        impl JitFilter {
            #[allow(clippy::too_many_arguments)]
            unsafe fn $name(
                &self,
                data_ptrs: &[*const f32],
                input: *const u8,
                output: *mut u8,
                len: i64,
                thresholds: &[f32],
            ) {
                type Fn = unsafe extern "C" fn(
                    $( jit_call_impl!(@ptr_type $di), )*
                    *const u8, *mut u8, i64,
                    $( jit_call_impl!(@f32_type $ti), )*
                );
                let f: Fn = std::mem::transmute(self.fn_ptr);
                f(
                    $( data_ptrs[$di], )*
                    input, output, len,
                    $( thresholds[$ti], )*
                );
            }
        }
    };
    (@ptr_type $i:tt) => { *const f32 };
    (@f32_type $i:tt) => { f32 };
}

jit_call_impl!(call_1, 1, (0), (0));
jit_call_impl!(call_2, 2, (0, 1), (0, 1));
jit_call_impl!(call_3, 3, (0, 1, 2), (0, 1, 2));
jit_call_impl!(call_4, 4, (0, 1, 2, 3), (0, 1, 2, 3));
jit_call_impl!(call_5, 5, (0, 1, 2, 3, 4), (0, 1, 2, 3, 4));
jit_call_impl!(call_6, 6, (0, 1, 2, 3, 4, 5), (0, 1, 2, 3, 4, 5));
jit_call_impl!(call_7, 7, (0, 1, 2, 3, 4, 5, 6), (0, 1, 2, 3, 4, 5, 6));
jit_call_impl!(call_8, 8, (0, 1, 2, 3, 4, 5, 6, 7), (0, 1, 2, 3, 4, 5, 6, 7));
jit_call_impl!(call_9, 9, (0, 1, 2, 3, 4, 5, 6, 7, 8), (0, 1, 2, 3, 4, 5, 6, 7, 8));
jit_call_impl!(call_10, 10, (0, 1, 2, 3, 4, 5, 6, 7, 8, 9), (0, 1, 2, 3, 4, 5, 6, 7, 8, 9));

fn cmpop_to_floatcc(op: CmpOp) -> FloatCC {
    match op {
        CmpOp::Gt => FloatCC::GreaterThan,
        CmpOp::Ge => FloatCC::GreaterThanOrEqual,
        CmpOp::Lt => FloatCC::LessThan,
        CmpOp::Le => FloatCC::LessThanOrEqual,
    }
}

/// Compile a set of conditions into a native filter function.
///
/// The generated function:
/// 1. Loops over `len` cells
/// 2. Skips cells where `input[i] == 0`
/// 3. For each active cell, loads all indicator values, compares against
///    thresholds using branchless `band`, stores result in `output[i]`
///
/// NaN handling: Cranelift's `fcmp` with ordered comparisons (GreaterThan, etc.)
/// returns false for NaN operands per IEEE 754, matching `FusedCondition::check`.
pub fn compile_filter(conditions: &[JitCondition]) -> JitFilter {
    let n = conditions.len();
    assert!(n >= 1 && n <= 10, "compile_filter: 1-10 conditions, got {n}");

    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    flag_builder.set("is_pic", "false").unwrap();
    flag_builder.set("opt_level", "speed").unwrap();
    let isa = cranelift_native::builder()
        .expect("host ISA")
        .finish(settings::Flags::new(flag_builder))
        .unwrap();
    let jit_builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
    let mut module = JITModule::new(jit_builder);

    let ptr_ty = types::I64;
    let mut sig = module.make_signature();
    for _ in 0..n {
        sig.params.push(AbiParam::new(ptr_ty));
    }
    sig.params.push(AbiParam::new(ptr_ty)); // input
    sig.params.push(AbiParam::new(ptr_ty)); // output
    sig.params.push(AbiParam::new(types::I64)); // len
    for _ in 0..n {
        sig.params.push(AbiParam::new(types::F32));
    }

    let func_id = module
        .declare_function("jit_filter", Linkage::Local, &sig)
        .unwrap();
    let mut ctx = module.make_context();
    ctx.func.signature = sig.clone();

    let mut builder_ctx = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut builder_ctx);

        let entry = b.create_block();
        let hdr = b.create_block();
        let body = b.create_block();
        let skip = b.create_block();
        let check = b.create_block();
        let exit = b.create_block();

        b.switch_to_block(entry);
        b.append_block_params_for_function_params(entry);

        let p = |idx: usize| b.block_params(entry)[idx];

        let data_vals: Vec<Value> = (0..n).map(|i| p(i)).collect();
        let inp = p(n);
        let outp = p(n + 1);
        let len = p(n + 2);
        let thresh_vals: Vec<Value> = (0..n).map(|i| p(n + 3 + i)).collect();

        let zero = b.ins().iconst(types::I64, 0);
        b.ins().jump(hdr, &[zero]);

        b.switch_to_block(hdr);
        b.append_block_param(hdr, types::I64);
        let i = b.block_params(hdr)[0];
        let done = b.ins().icmp(IntCC::SignedGreaterThanOrEqual, i, len);
        b.ins().brif(done, exit, &[], body, &[]);

        b.switch_to_block(body);
        let inp_addr = b.ins().iadd(inp, i);
        let inp_val = b.ins().load(types::I8, MemFlags::new(), inp_addr, 0);
        let iz = b.ins().iconst(types::I8, 0);
        let is_zero = b.ins().icmp(IntCC::Equal, inp_val, iz);
        b.ins().brif(is_zero, skip, &[], check, &[]);

        b.switch_to_block(check);
        let four = b.ins().iconst(types::I64, 4);
        let byte_off = b.ins().imul(i, four);

        let mut cmp_results: Vec<Value> = Vec::with_capacity(n);
        for (idx, cond) in conditions.iter().enumerate() {
            let addr = b.ins().iadd(data_vals[idx], byte_off);
            let val = b.ins().load(types::F32, MemFlags::new(), addr, 0);
            let cc = cmpop_to_floatcc(cond.op);
            let cmp = b.ins().fcmp(cc, val, thresh_vals[idx]);
            cmp_results.push(cmp);
        }

        let mut result = cmp_results[0];
        for r in &cmp_results[1..] {
            result = b.ins().band(result, *r);
        }

        let out_addr = b.ins().iadd(outp, i);
        b.ins().store(MemFlags::new(), result, out_addr, 0);
        b.ins().jump(skip, &[]);

        b.switch_to_block(skip);
        let one = b.ins().iconst(types::I64, 1);
        let next_i = b.ins().iadd(i, one);
        b.ins().jump(hdr, &[next_i]);

        b.switch_to_block(exit);
        b.ins().return_(&[]);

        b.seal_all_blocks();
        b.finalize();
    }

    module.define_function(func_id, &mut ctx).unwrap();
    module.clear_context(&mut ctx);
    module.finalize_definitions().unwrap();

    let fn_ptr = module.get_finalized_function(func_id);

    JitFilter {
        n_conditions: n,
        fn_ptr: fn_ptr as *const u8,
        _module: module,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_and_run_single_condition() {
        let conds = vec![JitCondition {
            indicator: Indicator::Close,
            op: CmpOp::Gt,
            threshold: 5.0,
        }];
        let filter = compile_filter(&conds);

        let data: Vec<f32> = vec![3.0, 6.0, 5.0, 10.0];
        let input: Vec<u8> = vec![1, 1, 1, 1];
        let mut output: Vec<u8> = vec![0; 4];

        unsafe {
            filter.call_raw(
                &[data.as_ptr()],
                input.as_ptr(),
                output.as_mut_ptr(),
                4,
                &[5.0],
            );
        }

        assert_eq!(output, vec![0, 1, 0, 1]);
    }

    #[test]
    fn compile_and_run_three_conditions() {
        let conds = vec![
            JitCondition { indicator: Indicator::Close, op: CmpOp::Gt, threshold: 5.0 },
            JitCondition { indicator: Indicator::VolSma20, op: CmpOp::Gt, threshold: 100.0 },
            JitCondition { indicator: Indicator::Dist52w, op: CmpOp::Le, threshold: 0.25 },
        ];
        let filter = compile_filter(&conds);

        let d0: Vec<f32> = vec![10.0, 3.0, 8.0];
        let d1: Vec<f32> = vec![200.0, 200.0, 50.0];
        let d2: Vec<f32> = vec![0.1, 0.1, 0.1];
        let input: Vec<u8> = vec![1, 1, 1];
        let mut output: Vec<u8> = vec![0; 3];

        unsafe {
            filter.call_raw(
                &[d0.as_ptr(), d1.as_ptr(), d2.as_ptr()],
                input.as_ptr(),
                output.as_mut_ptr(),
                3,
                &[5.0, 100.0, 0.25],
            );
        }

        assert_eq!(output, vec![1, 0, 0]);
    }

    #[test]
    fn respects_input_mask() {
        let conds = vec![JitCondition {
            indicator: Indicator::Close,
            op: CmpOp::Gt,
            threshold: 5.0,
        }];
        let filter = compile_filter(&conds);

        let data: Vec<f32> = vec![10.0, 10.0, 10.0];
        let input: Vec<u8> = vec![1, 0, 1];
        let mut output: Vec<u8> = vec![0; 3];

        unsafe {
            filter.call_raw(
                &[data.as_ptr()],
                input.as_ptr(),
                output.as_mut_ptr(),
                3,
                &[5.0],
            );
        }

        assert_eq!(output, vec![1, 0, 1]);
    }

    #[test]
    fn nan_fails_condition() {
        let conds = vec![JitCondition {
            indicator: Indicator::Close,
            op: CmpOp::Gt,
            threshold: 5.0,
        }];
        let filter = compile_filter(&conds);

        let data: Vec<f32> = vec![f32::NAN, 10.0];
        let input: Vec<u8> = vec![1, 1];
        let mut output: Vec<u8> = vec![0; 2];

        unsafe {
            filter.call_raw(
                &[data.as_ptr()],
                input.as_ptr(),
                output.as_mut_ptr(),
                2,
                &[5.0],
            );
        }

        assert_eq!(output, vec![0, 1]);
    }
}
