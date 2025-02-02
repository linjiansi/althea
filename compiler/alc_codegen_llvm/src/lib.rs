mod ctx;

extern crate core;

use crate::ctx::CodegenLLVMCtx;
use alc_ast_lowering::{idx::Idx, ir, ir::ExprKind, ty, ty::Array};
use alc_command_option::CommandOptions;
use alc_diagnostic::{Diagnostic, FileId, Label, Result, Span};
use inkwell::{
    builder::Builder,
    context::Context,
    module::Module,
    targets::{CodeModel, FileType, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple},
    types::{AnyType, AnyTypeEnum, BasicType, BasicTypeEnum, FunctionType},
    values::{ArrayValue, BasicValue, BasicValueEnum, FunctionValue, IntValue, PointerValue, VectorValue},
    AddressSpace,
    OptimizationLevel,
};
use log::debug;
use std::path::{Path, PathBuf};

const PRINTF: &str = "printf";
const SOCKET: &str = "socket";
const BIND: &str = "bind";
const LISTEN: &str = "listen";
const ACCEPT: &str = "accept";
const RECV: &str = "recv";
const SEND: &str = "send";
const CLOSE: &str = "close";
const SNPRINTF: &str = "snprintf";
const STRLEN: &str = "strlen";
const HTONS: &str = "htons";
const MALLOC: &str = "malloc";
const FREE: &str = "free";

pub fn generate<'a>(
    command_options: &'a CommandOptions,
    file_id: FileId,
    ty_sess: &'a ty::TySess,
    ir: &'a ir::Ir,
) -> Result<()> {
    CodegenLLVM::generate(command_options, file_id, ty_sess, ir)
}

pub struct CodegenLLVM<'gen, 'ctx> {
    command_options: &'gen CommandOptions,
    file_id: FileId,
    context: &'ctx Context,
    builder: &'gen Builder<'ctx>,
    module: &'gen Module<'ctx>,
    ty_sess: &'gen ty::TySess,
    ir: &'gen ir::Ir,
}

impl<'gen, 'ctx> CodegenLLVM<'gen, 'ctx> {
    fn generate(
        command_options: &'gen CommandOptions,
        file_id: FileId,
        ty_sess: &'gen ty::TySess,
        ir: &'gen ir::Ir,
    ) -> Result<()> {
        let context = Context::create();
        let module = context.create_module("alc");
        let builder = context.create_builder();
        module.set_source_file_name(command_options.src_file_name());
        let ctx = CodegenLLVM {
            command_options,
            file_id,
            context: &context,
            builder: &builder,
            module: &module,
            ty_sess,
            ir,
        };
        ctx.bind_reserved_functions();
        for def in ir.defs.values() {
            ctx.bind_def(def);
        }
        for def in ir.defs.values() {
            let compiled_def = ctx.compile_def(def)?;
            if command_options.debug && !compiled_def.verify(false) {
                compiled_def.print_to_stderr();
                compiled_def.verify(true);
            }
        }
        debug!("{}", module.print_to_string().to_string());
        module.verify().map_err(|err| {
            Diagnostic::new_bug(
                "LLVM IR could not be verified",
                Label::new(file_id, Span::dummy(), format!("{}", err)),
            )
        })?;
        ctx.write_to_ll_file(&ctx.command_options.out)?;
        ctx.write_to_asm_file(&ctx.command_options.out)?;
        let obj_file = ctx.write_to_obj_file(&ctx.command_options.out)?;
        ctx.execute_linker(obj_file)
    }

    fn bind_def(&self, def: &ir::Def) -> FunctionValue<'ctx> {
        let fn_ty = self.compile_ty(def.ty).into_function_type();
        self.module.add_function(&def.name, fn_ty, None)
    }

    fn bind_reserved_functions(&self) {
        self.module.add_function(
            PRINTF,
            self.context.i32_type().fn_type(
                &[self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::Generic)
                    .as_basic_type_enum()
                    .into()],
                false,
            ),
            None,
        );
        self.module.add_function(
            SOCKET,
            self.context.i32_type().fn_type(
                &[
                    self.context.i32_type().as_basic_type_enum().into(),
                    self.context.i32_type().as_basic_type_enum().into(),
                    self.context.i32_type().as_basic_type_enum().into(),
                ],
                false,
            ),
            None,
        );
        self.module.add_function(
            BIND,
            self.context.i32_type().fn_type(
                &[
                    self.context.i32_type().as_basic_type_enum().into(),
                    self.context
                        .struct_type(
                            &[
                                self.context.i16_type().as_basic_type_enum(),
                                self.context.i8_type().array_type(14).as_basic_type_enum(),
                            ],
                            false,
                        )
                        .ptr_type(AddressSpace::Generic)
                        .as_basic_type_enum()
                        .into(),
                    self.context.i32_type().as_basic_type_enum().into(),
                ],
                false,
            ),
            None,
        );
        self.module.add_function(
            LISTEN,
            self.context.i32_type().fn_type(
                &[
                    self.context.i32_type().as_basic_type_enum().into(),
                    self.context.i32_type().as_basic_type_enum().into(),
                ],
                false,
            ),
            None,
        );
        self.module.add_function(
            ACCEPT,
            self.context.i32_type().fn_type(
                &[
                    self.context.i32_type().as_basic_type_enum().into(),
                    self.context
                        .struct_type(
                            &[
                                self.context.i16_type().as_basic_type_enum(),
                                self.context.i8_type().array_type(14).as_basic_type_enum(),
                            ],
                            false,
                        )
                        .ptr_type(AddressSpace::Generic)
                        .as_basic_type_enum()
                        .into(),
                    self.context
                        .i32_type()
                        .ptr_type(AddressSpace::Generic)
                        .as_basic_type_enum()
                        .into(),
                ],
                false,
            ),
            None,
        );
        self.module.add_function(
            RECV,
            self.context.i64_type().fn_type(
                &[
                    self.context.i32_type().as_basic_type_enum().into(),
                    self.context
                        .i8_type()
                        .ptr_type(AddressSpace::Generic)
                        .as_basic_type_enum()
                        .into(),
                    self.context.i64_type().as_basic_type_enum().into(),
                    self.context.i32_type().as_basic_type_enum().into(),
                ],
                false,
            ),
            None,
        );
        self.module.add_function(
            SEND,
            self.context.i64_type().fn_type(
                &[
                    self.context.i32_type().as_basic_type_enum().into(),
                    self.context
                        .i8_type()
                        .ptr_type(AddressSpace::Generic)
                        .as_basic_type_enum()
                        .into(),
                    self.context.i64_type().as_basic_type_enum().into(),
                    self.context.i32_type().as_basic_type_enum().into(),
                ],
                false,
            ),
            None,
        );
        self.module.add_function(
            CLOSE,
            self.context
                .i32_type()
                .fn_type(&[self.context.i32_type().as_basic_type_enum().into()], false),
            None,
        );
        self.module.add_function(
            SNPRINTF,
            self.context.i32_type().fn_type(
                &[
                    self.context
                        .i8_type()
                        .ptr_type(AddressSpace::Generic)
                        .as_basic_type_enum()
                        .into(),
                    self.context.i64_type().as_basic_type_enum().into(),
                    self.context
                        .i8_type()
                        .ptr_type(AddressSpace::Generic)
                        .as_basic_type_enum()
                        .into(),
                ],
                true,
            ),
            None,
        );
        self.module.add_function(
            STRLEN,
            self.context.i64_type().fn_type(
                &[self
                    .context
                    .i8_type()
                    .ptr_type(AddressSpace::Generic)
                    .as_basic_type_enum()
                    .into()],
                false,
            ),
            None,
        );
        self.module.add_function(
            HTONS,
            self.context
                .i16_type()
                .fn_type(&[self.context.i16_type().as_basic_type_enum().into()], false),
            None,
        );
        self.module.add_function(
            MALLOC,
            self.context
                .i8_type()
                .ptr_type(AddressSpace::Generic)
                .fn_type(&[self.context.i64_type().as_basic_type_enum().into()], false),
            None,
        );
        self.module.add_function(
            FREE,
            self.context.void_type().fn_type(
                &[self.context.i8_type().ptr_type(AddressSpace::Generic).into()],
                false,
            ),
            None,
        );
    }

    fn lookup_def(&self, def: ir::DefIdx, span: Span) -> Result<FunctionValue<'ctx>> {
        let name = &self.ir.defs.get(def).unwrap().name;
        self.module.get_function(name).ok_or_else(|| {
            Box::from(Diagnostic::new_bug(
                "attempt to reference unregistered def",
                Label::new(
                    self.file_id,
                    span,
                    format!("{} is not listed in the LLVM module", name),
                ),
            ))
        })
    }

    fn build_free(&self, ptr: PointerValue<'ctx>) {
        let ptr = self.builder.build_pointer_cast(
            ptr,
            self.context.i8_type().ptr_type(AddressSpace::Generic),
            "raw",
        );
        self.builder.build_call(
            self.module.get_function(FREE).unwrap(),
            &[ptr.as_basic_value_enum().into()],
            "free",
        );
    }

    #[allow(clippy::match_single_binding)]
    fn build_alloc(&self, ty: ty::Ty, name: &str, span: Span) -> Result<PointerValue<'ctx>> {
        let ty = self.compile_basic_ty_unboxed(ty);
        match self.command_options.gc {
            // TODO: GC
            _ => self.builder.build_malloc(ty, name).map_err(|err| {
                Box::from(Diagnostic::new_bug(
                    "failed to build malloc call",
                    Label::new(self.file_id, span, err),
                ))
            }),
        }
    }

    #[inline]
    fn compile_i8_literal(&self, literal: i8) -> IntValue<'ctx> {
        self.context
            .i8_type()
            .const_int(literal as u64, false)
            .const_neg()
    }

    #[inline]
    fn compile_i16_literal(&self, literal: i16) -> IntValue<'ctx> {
        self.context.i16_type().const_int(literal as u64, false)
    }

    #[inline]
    fn compile_i32_literal(&self, literal: i32) -> IntValue<'ctx> {
        self.context.i32_type().const_int(literal as u64, false)
    }

    #[inline]
    fn compile_i64_literal(&self, literal: i64) -> IntValue<'ctx> {
        self.context.i64_type().const_int(literal as u64, false)
    }

    #[inline]
    fn compile_array_literal(&self, element_ty: ty::Ty, elements: Vec<ExprKind>) -> ArrayValue<'ctx> {
        match &*self.ty_sess.ty_kind(element_ty) {
            ty::TyKind::I8 => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    if let ExprKind::I8Literal(literal) = element {
                        values.push(self.compile_i8_literal(literal))
                    }
                }
                self.context.i8_type().const_array(&values)
            }
            ty::TyKind::I16 => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    if let ExprKind::I16Literal(literal) = element {
                        values.push(self.compile_i16_literal(literal))
                    }
                }
                self.context.i16_type().const_array(&values)
            }
            ty::TyKind::I32 => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    if let ExprKind::I32Literal(literal) = element {
                        values.push(self.compile_i32_literal(literal))
                    }
                }
                self.context.i32_type().const_array(&values)
            }
            ty::TyKind::I64 => {
                let mut values = Vec::with_capacity(elements.len());
                for element in elements {
                    if let ExprKind::I64Literal(literal) = element {
                        values.push(self.compile_i64_literal(literal))
                    }
                }
                self.context.i64_type().const_array(&values)
            }
            _ => {
                panic!("unimplemented array literal type");
            }
        }
    }

    #[inline]
    fn compile_string_literal(&self, literal: &str) -> VectorValue<'ctx> {
        self.context.const_string(literal.as_bytes(), true)
    }

    #[inline]
    fn compile_variant_idx(&self, idx: ty::VariantIdx) -> IntValue<'ctx> {
        self.compile_i32_literal(idx.index() as i32)
    }

    #[inline]
    fn read_param(&self, function: FunctionValue<'ctx>, idx: ty::ParamIdx) -> BasicValueEnum<'ctx> {
        if let Some(param) = function.get_nth_param(idx.index() as u32) {
            param
        } else {
            panic!(
                "attempted to param with index {} when it does not exist",
                idx.index()
            )
        }
    }

    #[inline]
    unsafe fn gep(&self, ptr: PointerValue<'ctx>, idx: u64, name: &str) -> PointerValue<'ctx> {
        self.builder.build_in_bounds_gep(
            ptr,
            &[
                self.context.i32_type().const_int(0, false),
                self.context.i32_type().const_int(idx, false),
            ],
            name,
        )
    }

    fn mark_ptr(&self, ptr: PointerValue<'ctx>, ty: ty::Ty) -> PointerValue<'ctx> {
        if self.ty_sess.ty_kind(ty).is_enum() {
            unsafe { self.gep(ptr, 2, "mark_ptr") }
        } else if self.ty_sess.ty_kind(ty).is_struct() {
            unsafe {
                self.gep(
                    ptr,
                    self.ty_sess.ty_kind(ty).field_count().unwrap() as u64,
                    "mark_ptr",
                )
            }
        } else {
            panic!("atttempted to read mark of a type that wasn't a struct or an enum")
        }
    }

    fn write_mark(&self, ptr: PointerValue<'ctx>, ty: ty::Ty, mark: bool) {
        self.builder.build_store(
            self.mark_ptr(ptr, ty),
            self.context
                .custom_width_int_type(1)
                .const_int(u64::from(mark), false),
        );
    }

    fn enum_discriminant_ptr(&self, ptr: PointerValue<'ctx>, ty: ty::Ty) -> PointerValue<'ctx> {
        if self.ty_sess.ty_kind(ty).is_enum() {
            unsafe { self.gep(ptr, 0, "discriminant_ptr") }
        } else {
            panic!("attempted to read discriminant of type that is not an enum")
        }
    }

    fn enum_body_ptr(&self, ptr: PointerValue<'ctx>, ty: ty::Ty) -> PointerValue<'ctx> {
        if self.ty_sess.ty_kind(ty).is_enum() {
            unsafe { self.gep(ptr, 1, "body_ptr") }
        } else {
            panic!("attempted to read body of type that is not an enum")
        }
    }

    #[inline]
    fn read_enum_discriminant(&self, ptr: PointerValue<'ctx>, ty: ty::Ty) -> Result<BasicValueEnum<'ctx>> {
        Ok(self
            .builder
            .build_load(self.enum_discriminant_ptr(ptr, ty), "discriminant"))
    }

    fn read_enum_body(
        &self,
        ptr: PointerValue<'ctx>,
        ty: ty::Ty,
        idx: ty::VariantIdx,
    ) -> BasicValueEnum<'ctx> {
        let variant_ty = self.ty_sess.ty_kind(ty).variant_ty(idx).unwrap();
        let body_ptr = self.enum_body_ptr(ptr, ty);
        if variant_ty != self.ty_sess.make_i64() {
            let uncast_body = self.builder.build_load(body_ptr, "body");
            self.builder
                .build_int_to_ptr(
                    uncast_body.into_int_value(),
                    self.compile_basic_ty(variant_ty).into_pointer_type(),
                    "body",
                )
                .into()
        } else {
            self.builder.build_load(body_ptr, "body")
        }
    }

    #[inline]
    fn write_enum_discriminant(&self, ptr: PointerValue<'ctx>, ty: ty::Ty, idx: ty::VariantIdx) {
        self.builder.build_store(
            self.enum_discriminant_ptr(ptr, ty),
            self.context.i64_type().const_int(idx.index() as u64, false),
        );
    }

    fn write_enum_body(
        &self,
        ptr: PointerValue<'ctx>,
        ty: ty::Ty,
        idx: ty::VariantIdx,
        val: BasicValueEnum<'ctx>,
    ) {
        let variant_ty = self.ty_sess.ty_kind(ty).variant_ty(idx).unwrap();
        let body_ptr = self.enum_body_ptr(ptr, ty);
        let cast_body = if variant_ty != self.ty_sess.make_i64() {
            self.builder
                .build_ptr_to_int(val.into_pointer_value(), self.context.i64_type(), "cast_body")
                .into()
        } else {
            val
        };
        self.builder.build_store(body_ptr, cast_body);
    }

    fn struct_field_ptr(&self, ptr: PointerValue<'ctx>, ty: ty::Ty, idx: ty::FieldIdx) -> PointerValue<'ctx> {
        if self.ty_sess.ty_kind(ty).field_ty(idx).is_some() {
            unsafe { self.gep(ptr, idx.index() as u64, &format!("field_{}_ptr", idx.index())) }
        } else {
            panic!(
                "attempted to access field with index {} when it does not exist",
                idx.index()
            )
        }
    }

    #[inline]
    fn write_struct_field(
        &self,
        ptr: PointerValue<'ctx>,
        ty: ty::Ty,
        idx: ty::FieldIdx,
        val: BasicValueEnum<'ctx>,
    ) {
        self.builder.build_store(self.struct_field_ptr(ptr, ty, idx), val);
    }

    #[inline]
    fn read_struct_field(
        &self,
        ptr: PointerValue<'ctx>,
        ty: ty::Ty,
        idx: ty::FieldIdx,
    ) -> BasicValueEnum<'ctx> {
        self.builder.build_load(
            self.struct_field_ptr(ptr, ty, idx),
            &format!("field_{}", idx.index()),
        )
    }

    fn compile_basic_ty_unboxed(&self, ty: ty::Ty) -> BasicTypeEnum<'ctx> {
        match &*self.ty_sess.ty_kind(ty) {
            ty::TyKind::I8 => self.context.i8_type().into(),
            ty::TyKind::I16 => self.context.i16_type().into(),
            ty::TyKind::I32 => self.context.i32_type().into(),
            ty::TyKind::I64 => self.context.i64_type().into(),
            ty::TyKind::String => self.context.i8_type().ptr_type(AddressSpace::Generic).into(),
            ty::TyKind::Array(Array { element_ty: _, size }) => {
                self.context.i8_type().array_type(*size as u32).into()
            }
            ty::TyKind::Enum(_) => {
                let field_tys = vec![self.context.i64_type().into(), self.context.i64_type().into()];
                // TODO: GC
                self.context.struct_type(field_tys.as_slice(), false).into()
            }
            ty::TyKind::Struct(ty::Struct { fields }) => {
                let field_tys = fields
                    .values()
                    .map(|ty| self.compile_basic_ty(*ty))
                    .collect::<Vec<_>>();
                // TODO: GC
                self.context.struct_type(field_tys.as_slice(), false).into()
            }
            _ => panic!("attempted to compile function type as basic type"),
        }
    }

    fn compile_basic_ty(&self, ty: ty::Ty) -> BasicTypeEnum<'ctx> {
        let compiled_ty_unboxed = self.compile_basic_ty_unboxed(ty);
        if self.ty_sess.ty_kind(ty).is_i8()
            || self.ty_sess.ty_kind(ty).is_i16()
            || self.ty_sess.ty_kind(ty).is_i32()
            || self.ty_sess.ty_kind(ty).is_i64()
            || self.ty_sess.ty_kind(ty).is_string()
            || self.ty_sess.ty_kind(ty).is_array()
        {
            compiled_ty_unboxed
        } else {
            compiled_ty_unboxed.ptr_type(AddressSpace::Generic).into()
        }
    }

    fn compile_fn_ty(&self, prototype: &ty::Prototype) -> FunctionType<'ctx> {
        let mut param_tys = Vec::with_capacity(prototype.params.len());
        for ty in prototype.params.values() {
            let compiled_ty = self.compile_basic_ty(*ty).into();
            param_tys.push(compiled_ty);
        }
        // TODO
        self.compile_basic_ty(prototype.return_ty)
            .fn_type(param_tys.as_slice(), false)
    }

    fn compile_ty(&self, ty: ty::Ty) -> AnyTypeEnum<'ctx> {
        match &*self.ty_sess.ty_kind(ty) {
            ty::TyKind::Fn(prototype) => self.compile_fn_ty(prototype).into(),
            _ => self.compile_basic_ty(ty).as_any_type_enum(),
        }
    }

    fn compile_def(&self, def: &ir::Def) -> Result<FunctionValue<'ctx>> {
        CodegenLLVMCtx::compile_def(self, def)
    }

    fn write_to_ll_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut output_path = PathBuf::from(path.as_ref());
        output_path.set_extension("ll");

        self.module.print_to_file(output_path).map_err(|e| {
            Box::from(Diagnostic::new_error(
                "failed to write LLVM IR to file",
                Label::new(self.file_id, Span::dummy(), format!("{}", e)),
            ))
        })
    }

    fn write_to_obj_file<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let target_machine = self.get_target_machine()?;

        let mut output_path = PathBuf::from(path.as_ref());
        output_path.set_extension("o");

        let compile_result = target_machine
            .write_to_file(self.module, FileType::Object, output_path.as_path())
            .map_err(|e| {
                Diagnostic::new_error(
                    "failed to write object file",
                    Label::new(self.file_id, Span::dummy(), format!("{}", e)),
                )
            });

        match compile_result {
            Ok(_) => Ok(output_path),
            Err(diagnostic) => Err(Box::from(diagnostic)),
        }
    }

    fn write_to_asm_file<P: AsRef<Path>>(&self, path: P) -> Result<PathBuf> {
        let target_machine = self.get_target_machine()?;

        let mut output_path = PathBuf::from(path.as_ref());
        output_path.set_extension("s");

        let compile_result = target_machine
            .write_to_file(self.module, FileType::Assembly, output_path.as_path())
            .map_err(|e| {
                Diagnostic::new_error(
                    "failed to write assembly file",
                    Label::new(self.file_id, Span::dummy(), format!("{}", e)),
                )
            });

        match compile_result {
            Ok(_) => Ok(output_path),
            Err(diagnostic) => Err(Box::from(diagnostic)),
        }
    }

    fn get_target_machine(&self) -> Result<TargetMachine> {
        Target::initialize_native(&InitializationConfig::default()).unwrap();
        let triple = match &self.command_options.triple {
            Some(triple) => TargetTriple::create(triple.as_str()),
            None => TargetMachine::get_default_triple(),
        };
        let target = match Target::from_triple(&triple) {
            Ok(target) => target,
            Err(e) => {
                return Err(Box::from(Diagnostic::new_error(
                    "failed to get target",
                    Label::new(self.file_id, Span::dummy(), format!("{}", e)),
                )))
            }
        };

        let cpu = match &self.command_options.cpu_name {
            Some(cpu) => cpu.to_string(),
            None => TargetMachine::get_host_cpu_name().to_string(),
        };
        let features = match &self.command_options.cpu_features {
            Some(features) => features.to_string(),
            None => TargetMachine::get_host_cpu_features().to_string(),
        };

        let opt_level = OptimizationLevel::Default;
        let reloc_mode = RelocMode::Default;
        let code_model = CodeModel::Default;

        target
            .create_target_machine(
                &triple,
                cpu.as_str(),
                features.as_str(),
                opt_level,
                reloc_mode,
                code_model,
            )
            .ok_or_else(|| {
                Box::from(Diagnostic::new_error(
                    "failed to get target machine",
                    Label::new(self.file_id, Span::dummy(), ""),
                ))
            })
    }

    fn execute_linker<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let cc = std::env::var("CC").unwrap_or_else(|_| "gcc".into());
        let extension = if cfg!(windows) { "exe" } else { "" };

        let mut output_path = PathBuf::from(path.as_ref());
        output_path.set_extension(extension);

        let command_output = std::process::Command::new(cc)
            .args(vec![
                path.as_ref().as_os_str(),
                std::ffi::OsStr::new("-o"),
                output_path.as_os_str(),
            ])
            .output()
            .unwrap();

        let status = command_output.status.code().unwrap();
        let stderr = String::from_utf8(command_output.stderr).unwrap();

        if status != 0 {
            return Err(Box::from(Diagnostic::new_error(
                "failed to execute linker",
                Label::new(self.file_id, Span::dummy(), stderr),
            )));
        }

        Ok(())
    }
}
