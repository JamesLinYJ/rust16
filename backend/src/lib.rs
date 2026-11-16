#![feature(rustc_private)]
extern crate rustc_abi;
extern crate rustc_attr_ir;
extern crate rustc_codegen_ssa;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_metadata;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_structures;
use rust16::machine::Segment;
use rustc_codegen_ssa::{CompiledModules, CrateInfo, traits::CodegenBackend};
use rustc_middle::{dep_graph::WorkProductMap, ty::TyCtxt};
use rustc_session::{
    IncrCompSession, Session,
    config::{OutputFilenames, OutputType},
};
use std::any::Any;
mod layout;
mod mir;

fn symbol<'tcx>(tcx: TyCtxt<'tcx>, instance: rustc_middle::ty::Instance<'tcx>) -> String {
    let id = instance.def_id();
    let attrs = tcx.codegen_fn_attrs(id);
    if let Some(name) = attrs.symbol_name {
        return name.to_string();
    }
    if tcx.is_foreign_item(id) || attrs.contains_extern_indicator() {
        return tcx.item_name(id).to_string();
    }
    tcx.symbol_name(instance).name.to_string()
}
struct Dos16;
impl CodegenBackend for Dos16 {
    fn name(&self) -> &'static str {
        "rust16"
    }
    fn target_config(&self, _: &rustc_session::EarlySession) -> rustc_codegen_ssa::TargetConfig {
        rustc_codegen_ssa::TargetConfig {
            internal_target_features: [rustc_span::Symbol::intern("soft-float")]
                .into_iter()
                .collect(),
            has_reliable_f16b: false,
            has_reliable_f16: false,
            has_reliable_f16_math: false,
            has_reliable_f128: false,
            has_reliable_f128_math: false,
        }
    }
    fn target_cpu(&self, sess: &Session) -> String {
        sess.opts
            .cg
            .target_cpu
            .clone()
            .unwrap_or_else(|| sess.target.cpu.to_string())
    }
    fn codegen_crate<'tcx>(&self, tcx: TyCtxt<'tcx>) -> Box<dyn Any> {
        let sess = tcx.sess;
        if sess.target.arch.to_string() != "x86" || sess.target.pointer_width != 16 {
            sess.dcx()
                .fatal("rust16 requires the 16-bit x86 DOS target");
        }
        let cpu = match self.target_cpu(sess).as_str() {
            "8086" | "8088" => 86,
            "286" => 286,
            "386" => 386,
            other => sess
                .dcx()
                .fatal(format!("unsupported real-mode CPU {other}")),
        };
        let mut seg = Segment::default();
        use rustc_hir::def::DefKind;
        use rustc_middle::ty::{EarlyBinder, Instance, TypingEnv};
        let standalone = sess.opts.output_types.contains_key(&OutputType::Object);
        let executable = tcx
            .crate_types()
            .contains(&rustc_structures::CrateType::Executable);
        if !standalone && !executable {
            return Box::new(Vec::<u8>::new());
        }
        let mut queue = Vec::new();
        for local in tcx.hir_body_owners() {
            let id = local.to_def_id();
            if matches!(tcx.def_kind(id), DefKind::Fn | DefKind::AssocFn)
                && tcx.generics_of(id).count() == 0
                && (tcx.codegen_fn_attrs(id).contains_extern_indicator()
                    || (standalone && tcx.visibility(id).is_public()))
            {
                queue.push(Instance::mono(tcx, id));
            }
        }
        let mut emitted = std::collections::HashSet::new();
        while let Some(instance) = queue.pop() {
            if !emitted.insert(instance) {
                continue;
            }
            let id = instance.def_id();
            let name = symbol(tcx, instance);
            let body = instance.instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(tcx, tcx.instance_mir(instance.def).clone()),
            );
            if let Err(e) = mir::function(tcx, &body, &name, cpu, &mut seg, &mut queue) {
                sess.dcx()
                    .span_fatal(tcx.def_span(id), format!("{name}: {e}"));
            }
        }
        Box::new(seg.finish().unwrap_or_else(|e| sess.dcx().fatal(e)))
    }
    fn join_codegen(
        &self,
        ongoing: Box<dyn Any>,
        sess: &Session,
        _: Option<&IncrCompSession>,
        out: &OutputFilenames,
        _: &CrateInfo,
    ) -> (CompiledModules, WorkProductMap) {
        let obj = *ongoing.downcast::<Vec<u8>>().unwrap();
        let wants_object = sess.opts.output_types.contains_key(&OutputType::Object);
        let linking = sess.opts.output_types.contains_key(&OutputType::Exe);
        if !wants_object && !linking {
            sess.dcx()
                .fatal("rust16 supports OMF object or executable output");
        }
        let obj_output = out.path(OutputType::Object);
        if wants_object && obj_output.is_stdout() {
            use std::io::Write;
            if let Err(e) = std::io::stdout().lock().write_all(&obj) {
                sess.dcx().fatal(format!("writing OMF object: {e}"));
            }
            if !linking {
                return (
                    CompiledModules {
                        modules: vec![],
                        allocator_module: None,
                    },
                    Default::default(),
                );
            }
        }
        let path = if wants_object {
            out.path(OutputType::Object)
                .file_for_writing(out, OutputType::Object, "dos16")
        } else {
            out.temp_path_for_cgu(OutputType::Object, "dos16")
        };
        if let Err(e) = std::fs::write(&path, obj) {
            sess.dcx().fatal(format!("writing OMF object: {e}"));
        }
        let module = rustc_codegen_ssa::CompiledModule {
            name: "dos16".into(),
            kind: rustc_codegen_ssa::ModuleKind::Regular,
            object: Some(path),
            global_asm_object: None,
            dwarf_object: None,
            bytecode: None,
            assembly: None,
            llvm_ir: None,
        };
        (
            CompiledModules {
                modules: vec![module],
                allocator_module: None,
            },
            Default::default(),
        )
    }
    fn link(
        &self,
        sess: &Session,
        modules: CompiledModules,
        info: CrateInfo,
        metadata: rustc_metadata::EncodedMetadata,
        out: &OutputFilenames,
    ) {
        if !sess.opts.output_types.contains_key(&OutputType::Exe) {
            return;
        }
        for kind in &info.crate_types {
            let exe = rustc_session::output::out_filename(sess, *kind, out, info.local_crate_name);
            if *kind == rustc_structures::CrateType::Rlib {
                // MIR is instantiated when the consuming executable is compiled.
                let data = metadata.full();
                if data.is_empty() {
                    sess.dcx().fatal("missing encoded Rust library metadata");
                }
                let mut archive = format!(
                    "!<arch>\n{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`\n",
                    "lib.rmeta/",
                    0,
                    0,
                    0,
                    "644",
                    data.len()
                )
                .into_bytes();
                archive.extend_from_slice(data);
                if data.len() % 2 != 0 {
                    archive.push(b'\n');
                }
                std::fs::write(exe.as_path(), archive)
                    .unwrap_or_else(|e| sess.dcx().fatal(format!("writing rlib: {e}")));
                continue;
            }
            if *kind != rustc_structures::CrateType::Executable {
                sess.dcx().fatal(format!("unsupported DOS output {kind:?}"));
            }
            let mut cmd = std::process::Command::new(
                std::env::var_os("RUST16_LINKER").unwrap_or_else(|| "link16".into()),
            );
            let platform = std::env::var("RUST16_PLATFORM").unwrap_or_else(|_| "dos".into());
            let mut generated = None;
            match platform.as_str() {
                "dos" => {
                    cmd.arg(std::env::var_os("RUST16_RUNTIME").unwrap_or_else(|| "RT.OBJ".into()));
                }
                "bios-loader" | "bios-kernel" => {
                    let file = out.temp_path_for_cgu(OutputType::Object, "firmware16");
                    let firmware = rust16::firmware::object(platform == "bios-kernel")
                        .unwrap_or_else(|e| sess.dcx().fatal(e));
                    std::fs::write(&file, firmware)
                        .unwrap_or_else(|e| sess.dcx().fatal(format!("firmware object: {e}")));
                    cmd.arg(&file);
                    generated = Some(file);
                }
                _ => sess
                    .dcx()
                    .fatal(format!("unknown real-mode runtime {platform}")),
            }
            for m in &modules.modules {
                cmd.arg(m.object.as_ref().unwrap());
            }
            cmd.arg("-o").arg(exe.as_path());
            let status = cmd
                .status()
                .unwrap_or_else(|e| sess.dcx().fatal(format!("starting link16: {e}")));
            if let Some(path) = generated {
                if !sess.opts.cg.save_temps {
                    let _ = std::fs::remove_file(path);
                }
            }
            if !status.success() {
                sess.dcx().fatal(format!("link16 failed: {status}"));
            }
        }
    }
}
#[unsafe(no_mangle)]
pub fn __rustc_codegen_backend() -> Box<dyn CodegenBackend> {
    Box::new(Dos16)
}
