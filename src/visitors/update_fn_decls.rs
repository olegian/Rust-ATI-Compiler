/* Defines the visitor which edits all type signatures and definitions to
 * wrap primitive types T into TaggedValue<T> (defined in ati.rs).
 * After this pass, all declared types should be in a form which allows
 * unique tags to be carried alongside values.
*/
use rustc_ast::mut_visit::{self, MutVisitor};
use rustc_ast::{self as ast, GenericArgs};
use rustc_span::{DUMMY_SP, Ident};

use crate::common;
use crate::common::CanBeTupled;
use crate::types::ati_info::{FirstPassInfo, FunctionSignatures, ReceiverKind};

pub struct UpdateFnDeclsVisitor<'a> {
    first_pass: &'a FirstPassInfo,
    fn_sigs: Option<FunctionSignatures>,
}

impl<'a> MutVisitor for UpdateFnDeclsVisitor<'a> {
    /// Updates type annotations on `let` bindings so that primitives like `u32`
    /// become `Tagged<u32>` in sync with the rest of the instrumentation.
    // TODO: move this operation to Tuple Lits Visitor?
    fn visit_local(&mut self, local: &mut ast::Local) {
        if let Some(ty) = &mut local.ty {
            self.recursively_tuple_type(ty);
        }
        mut_visit::walk_local(self, local);
    }

    /// Updates turbofish generics in function/method calls so that, e.g.,
    /// `f::<u32>(x)` becomes `f::<Tagged<u32>>(x)`.
    // TODO: move this operation to Tuple Lits Visitor?
    fn visit_expr(&mut self, expr: &mut ast::Expr) {
        match &mut expr.kind {
            ast::ExprKind::Call(func, _) => {
                if let ast::ExprKind::Path(_, path) = &mut func.kind {
                    for segment in path.segments.iter_mut() {
                        self.tuple_generic_args_in_segment(segment);
                    }
                }
            }
            ast::ExprKind::MethodCall(box ast::MethodCall { seg, .. }) => {
                self.tuple_generic_args_in_segment(seg);
            }
            _ => {}
        }
        mut_visit::walk_expr(self, expr);
    }

    /// Converts all function signatures and top level type definitions (structs)
    /// to thier tagged variants. Specifically modifies all parameter types to
    /// be TaggedValues if necessary, alongside returns.
    fn visit_item(&mut self, item: &mut ast::Item) {
        match &mut item.kind {
            // Tags all input and return types that can be tupled in fn sigs
            ast::ItemKind::Fn(box ast::Fn {
                ident,
                sig: ast::FnSig { decl, .. },
                body,
                ..
            }) => {
                if !self.first_pass.is_fn_ident_tracked(ident) {
                    // we have previously decided that this function is not tracked and shouldn't be instrumented
                    return;
                }

                // adds a TaggedValue<*> around all taggable types, recursively
                for param in &mut decl.inputs {
                    self.recursively_tuple_type(&mut param.ty);
                }

                // we know this function is tracked, at some point, it will need a stub made
                // which requires knowledge of it's name, inputs, and outputs. Record all that info
                let orig_ident = ident.as_str();
                if let ast::FnRetTy::Ty(return_type) = &mut decl.output {
                    // do the recursive wrapping to the return type if one exists
                    self.recursively_tuple_type(return_type);
                    self.fn_sigs.as_mut().unwrap().register_fn_sig(
                        &orig_ident,
                        decl.inputs.iter().collect(),
                        Some(return_type),
                    );
                } else {
                    self.fn_sigs.as_mut().unwrap().register_fn_sig(
                        &orig_ident,
                        decl.inputs.iter().collect(),
                        None,
                    );
                }

                // rename the function so the stub can take its place
                let unstubbed = self
                    .fn_sigs
                    .as_mut()
                    .unwrap()
                    .reserve_unstubbed_name(&orig_ident);
                *ident = Ident::from_str(&unstubbed);

                // Walk the body to update type hints in let bindings and turbofish.
                if let Some(body) = body {
                    mut_visit::walk_block(self, body);
                }
            }

            // Tags all values in struct defs that can be tupled
            // FIXME: generics????
            ast::ItemKind::Struct(ident, generics, ast::VariantData::Struct { fields, .. }) => {
                for field_def in fields.iter_mut() {
                    self.recursively_tuple_type(&mut field_def.ty);
                }

                self.fn_sigs
                    .as_mut()
                    .unwrap()
                    .register_struct_def(ident.as_str(), &fields[..]);
            }

            // Tags all values in enum variant fields that can be tupled
            ast::ItemKind::Enum(ident, _, ast::EnumDef { variants }) => {
                let enum_name = ident.as_str().to_string();
                for variant in variants.iter_mut() {
                    match &mut variant.data {
                        ast::VariantData::Struct { fields, .. } => {
                            for field in fields.iter_mut() {
                                self.recursively_tuple_type(&mut field.ty);
                            }
                        }
                        ast::VariantData::Tuple(fields, _) => {
                            for field in fields.iter_mut() {
                                self.recursively_tuple_type(&mut field.ty);
                            }
                        }
                        ast::VariantData::Unit(_) => {}
                    }
                }
                self.fn_sigs
                    .as_mut()
                    .unwrap()
                    .register_enum_def(&enum_name, &variants[..]);
            }

            // Tags tracked methods in impl blocks and registers their signatures for stub creation.
            // Each tracked method is renamed to `method_unstubbed`; a matching stub
            // (retaining the original name) is generated later by `create_stub_items`.
            // Generic impl blocks are not yet supported and are silently skipped.
            ast::ItemKind::Impl(ast::Impl {
                generics,
                self_ty,
                items,
                ..
            }) => {
                if !generics.params.is_empty() {
                    // TODO: support generic impl blocks
                    return;
                }

                let type_name = common::get_type_string(self_ty);

                for assoc_item in items.iter_mut() {
                    let ast::AssocItemKind::Fn(box ast::Fn {
                        ident,
                        sig: ast::FnSig { decl, .. },
                        body,
                        ..
                    }) = &mut assoc_item.kind
                    else {
                        continue;
                    };

                    if !self.first_pass.is_fn_ident_tracked(ident) {
                        continue;
                    }

                    let method_name = ident.as_str().to_string();
                    let receiver = UpdateFnDeclsVisitor::determine_receiver_kind(&decl.inputs);

                    // tag all non-self parameter types
                    for param in &mut decl.inputs {
                        if !UpdateFnDeclsVisitor::is_self_param(param) {
                            self.recursively_tuple_type(&mut param.ty);
                        }
                    }

                    // collect non-self params for stub registration
                    let non_self_params: Vec<&ast::Param> = decl
                        .inputs
                        .iter()
                        .filter(|p| !UpdateFnDeclsVisitor::is_self_param(p))
                        .collect();

                    if let ast::FnRetTy::Ty(ret_ty) = &mut decl.output {
                        self.recursively_tuple_type(ret_ty);
                        self.fn_sigs.as_mut().unwrap().register_method_sig(
                            &type_name,
                            &method_name,
                            receiver,
                            non_self_params,
                            Some(ret_ty),
                        );
                    } else {
                        self.fn_sigs.as_mut().unwrap().register_method_sig(
                            &type_name,
                            &method_name,
                            receiver,
                            non_self_params,
                            None,
                        );
                    }

                    let unstubbed = self.fn_sigs.as_mut().unwrap().reserve_unstubbed_name_for(
                        &format!("{type_name}::{method_name}"),
                        &method_name,
                    );
                    *ident = Ident::from_str(&unstubbed);

                    // walk the method body to update type hints
                    if let Some(body) = body {
                        mut_visit::walk_block(self, body);
                    }
                }
            }

            _ => {}
        }
    }
}

impl<'a> UpdateFnDeclsVisitor<'a> {
    /// Constructor. `module_path` is used to qualify runtime site names.
    pub fn new(first_pass: &'a FirstPassInfo, module_path: &str) -> Self {
        let mut fn_sigs = FunctionSignatures::default();
        fn_sigs.set_module_path(module_path);
        Self {
            first_pass,
            fn_sigs: Some(fn_sigs),
        }
    }

    /// Pre-scans the crate to collect all function and method identifiers.
    /// Must be called before `visit_crate` so that `_unstubbed` rename
    /// collision detection works.
    pub fn collect_known_idents(&mut self, krate: &ast::Crate) {
        let fn_sigs = self.fn_sigs.as_mut().unwrap();
        for item in &krate.items {
            match &item.kind {
                ast::ItemKind::Fn(box ast::Fn { ident, .. }) => {
                    fn_sigs.add_known_ident(ident.as_str());
                }
                ast::ItemKind::Impl(ast::Impl { items, .. }) => {
                    for assoc_item in items {
                        if let ast::AssocItemKind::Fn(box ast::Fn { ident, .. }) = &assoc_item.kind
                        {
                            fn_sigs.add_known_ident(ident.as_str());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Pulls out all information about function signatures that this visitor
    /// modified. Panics if invoked before the pass is performed.
    pub fn get_new_fn_signatures(&mut self) -> FunctionSignatures {
        self.fn_sigs.take().expect("FnSigs was already taken!")
    }

    /// Directly modifies a type T into a TaggedValue<T> in place,
    /// assumes that T is known to be tupleable.
    fn tuple_type(&self, old_type: &mut ast::Ty) {
        old_type.kind = ast::TyKind::Path(
            None,
            ast::Path {
                segments: [ast::PathSegment {
                    ident: Ident::from_str("Tagged"),
                    id: ast::DUMMY_NODE_ID,
                    args: Some(Box::new(ast::AngleBracketed(ast::AngleBracketedArgs {
                        span: DUMMY_SP,
                        args: [ast::AngleBracketedArg::Arg(ast::GenericArg::Type(
                            Box::new(old_type.clone()),
                        ))]
                        .into(),
                    }))),
                }]
                .into(),
                span: DUMMY_SP,
                tokens: None,
            },
        );
    }

    fn tuple_slice(&self, slice_ty: &mut ast::Ty) {
        // println!("{:#?}", slice_ty);
        let mut tagged_slice = ast::PathSegment::from_ident(Ident::from_str("Tagged"));
        tagged_slice.args = Some(Box::new(GenericArgs::AngleBracketed(
            ast::AngleBracketedArgs {
                span: DUMMY_SP,
                args: [ast::AngleBracketedArg::Arg(ast::GenericArg::Type(
                    Box::new(slice_ty.clone()),
                ))]
                .into(),
            },
        )));

        let mut outer_ref = slice_ty.clone();
        let ast::TyKind::Ref(lt, mut_ty) = &mut outer_ref.kind else {
            unimplemented!("Slice behind non-reference pointer is currently unimplemented")
        };

        mut_ty.ty.kind = ast::TyKind::Path(
            None,
            ast::Path {
                span: DUMMY_SP,
                segments: [tagged_slice].into(),
                tokens: None,
            },
        );

        slice_ty.kind = outer_ref.kind;
    }

    fn tuple_array(&self, array_ty: &mut ast::Ty) {
        let mut tagged_array = ast::PathSegment::from_ident(Ident::from_str("Tagged"));
        tagged_array.args = Some(Box::new(GenericArgs::AngleBracketed(
            ast::AngleBracketedArgs {
                span: DUMMY_SP,
                args: [ast::AngleBracketedArg::Arg(ast::GenericArg::Type(
                    Box::new(array_ty.clone()),
                ))]
                .into(),
            },
        )));

        array_ty.kind = ast::TyKind::Path(
            None,
            ast::Path {
                span: DUMMY_SP,
                segments: [tagged_array].into(),
                tokens: None,
            },
        );
    }

    /// Applies `recursively_tuple_type` to all type generic arguments in a path segment,
    /// handling turbofish annotations like `func::<u32>` → `func::<Tagged<u32>>`.
    fn tuple_generic_args_in_segment(&self, segment: &mut ast::PathSegment) {
        let Some(ref mut boxed_args) = segment.args else {
            return;
        };
        let ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs { ref mut args, .. }) =
            **boxed_args
        else {
            return;
        };
        for arg in args.iter_mut() {
            if let ast::AngleBracketedArg::Arg(ast::GenericArg::Type(ty)) = arg {
                self.recursively_tuple_type(ty);
            }
        }
    }

    /// returns true if param is a self receiver (`self`, `&self`, `&mut self`)
    fn is_self_param(param: &ast::Param) -> bool {
        matches!(param.ty.peel_refs().kind, ast::TyKind::ImplicitSelf)
    }

    /// classifies the self receiver kind from an impl method's parameter list
    // FIXME: combine with above, but also then restructure the usage which is annoying...
    fn determine_receiver_kind(params: &[ast::Param]) -> ReceiverKind {
        let Some(first) = params.first() else {
            return ReceiverKind::None;
        };

        if !UpdateFnDeclsVisitor::is_self_param(first) {
            return ReceiverKind::None;
        }

        match &first.ty.kind {
            ast::TyKind::ImplicitSelf => ReceiverKind::Value,
            ast::TyKind::Ref(_, ast::MutTy { mutbl, .. }) => {
                if matches!(mutbl, ast::Mutability::Mut) {
                    ReceiverKind::RefMut
                } else {
                    ReceiverKind::Ref
                }
            }
            // Explicit `self: Self`, which should just be treated as a regular input param
            _ => ReceiverKind::Value,
        }
    }

    /// Searches through type `ty` to find and tuple all primitive types
    /// that should be tupled. Modifies the type in place.
    /// Strips off references (both & and &mut), acting on the actual referenced-types.
    fn recursively_tuple_type<'b>(&self, ty: &'b mut ast::Ty) {
        let peeled_type = common::peel_refs(ty);

        // base case, the type can just be tupled and no recursion is necessary
        if peeled_type.can_be_tupled() {
            self.tuple_type(peeled_type);
            return;
        }

        match &mut peeled_type.kind {
            rustc_ast::TyKind::Slice(inner_ty) => {
                self.recursively_tuple_type(inner_ty);
                self.tuple_slice(ty);
            }

            rustc_ast::TyKind::Array(inner_ty, _) => {
                self.recursively_tuple_type(inner_ty);
                self.tuple_array(ty);
            }

            rustc_ast::TyKind::Ptr(ast::MutTy { box ty, .. })
            | rustc_ast::TyKind::Ref(_, ast::MutTy { box ty, .. }) => {
                self.recursively_tuple_type(ty);
            }

            rustc_ast::TyKind::FnPtr(box ast::FnPtrTy {
                generic_params,
                decl: box ast::FnDecl { inputs, output },
                ..
            }) => {
                // tuple all generic types for this function pointer
                for generic in generic_params {
                    match &mut generic.kind {
                        rustc_ast::GenericParamKind::Type { default } => {
                            if let Some(ty) = default {
                                self.recursively_tuple_type(ty);
                            }
                        }
                        rustc_ast::GenericParamKind::Const { ty, .. } => {
                            self.recursively_tuple_type(ty);
                        }
                        rustc_ast::GenericParamKind::Lifetime => {}
                    }
                }

                // tuple all param input types
                for input in inputs {
                    self.recursively_tuple_type(&mut input.ty)
                }

                // tuple output type, if one exists
                if let ast::FnRetTy::Ty(box ty) = output {
                    self.recursively_tuple_type(ty);
                }
            }

            rustc_ast::TyKind::Tup(tys) => {
                for ty in tys {
                    self.recursively_tuple_type(ty);
                }
            }

            rustc_ast::TyKind::Path(_, ast::Path { segments, .. }) => {
                // traverse path::to::func() by segment, if any generics exist on any of the paths,
                // tuple those generic types
                for segment in segments.iter_mut() {
                    if let Some(box arg) = &mut segment.args {
                        match arg {
                            rustc_ast::GenericArgs::AngleBracketed(ast::AngleBracketedArgs {
                                args,
                                ..
                            }) => {
                                for arg in args.iter_mut() {
                                    match arg {
                                        rustc_ast::AngleBracketedArg::Arg(generic_arg) => {
                                            match generic_arg {
                                                rustc_ast::GenericArg::Type(ty) => {
                                                    self.recursively_tuple_type(ty);
                                                }
                                                rustc_ast::GenericArg::Const(_)
                                                | rustc_ast::GenericArg::Lifetime(_) => {}
                                            }
                                        }
                                        rustc_ast::AngleBracketedArg::Constraint(_) => {
                                            todo!("Constraint is a trait?")
                                        }
                                    }
                                }
                            }
                            rustc_ast::GenericArgs::Parenthesized(ast::ParenthesizedArgs {
                                inputs,
                                output,
                                ..
                            }) => {
                                for input in inputs {
                                    self.recursively_tuple_type(input);
                                }

                                if let ast::FnRetTy::Ty(box ty) = output {
                                    self.recursively_tuple_type(ty);
                                }
                            }
                            rustc_ast::GenericArgs::ParenthesizedElided(span) => {
                                panic!("this panic is probably fine to remove")
                            }
                        }
                    }
                }
            }

            // maybe impl later
            rustc_ast::TyKind::PinnedRef(_, _) => todo!(),
            rustc_ast::TyKind::Pat(_, _) => todo!(),

            // probably left untouched
            rustc_ast::TyKind::Infer => panic!(),
            rustc_ast::TyKind::TraitObject(_, _) => panic!(),
            rustc_ast::TyKind::Paren(_) => panic!(),
            rustc_ast::TyKind::UnsafeBinder(_) => panic!(),
            rustc_ast::TyKind::Never => panic!(),
            rustc_ast::TyKind::ImplTrait(_, _) => panic!(),
            rustc_ast::TyKind::ImplicitSelf => panic!(),
            rustc_ast::TyKind::MacCall(_) => panic!(),
            rustc_ast::TyKind::CVarArgs => panic!(),
            rustc_ast::TyKind::Dummy => panic!(),
            rustc_ast::TyKind::Err(_) => panic!(),
        };
    }
}
