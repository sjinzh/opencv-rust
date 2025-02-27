use std::borrow::Cow;
use std::collections::HashMap;

use once_cell::sync::Lazy;

use crate::class::ClassKind;
use crate::debug::NameDebug;
use crate::field::{Field, FieldDesc};
use crate::func::{FuncCppBody, FuncDesc, FuncKind, ReturnKind};
use crate::type_ref::{Constness, CppNameStyle, ExternDir, FishStyle, NameStyle, TypeRef, TypeRefDesc, TypeRefKind};
use crate::writer::rust_native::func::cpp_return_map;
use crate::{settings, Class, CompiledInterpolation, Element, Func, IteratorExt, NamePool, StrExt};

use super::element::{DefaultRustNativeElement, RustElement};
use super::type_ref::TypeRefExt;
use super::RustNativeGeneratedElement;

fn gen_rust_class(c: &Class, opencv_version: &str) -> String {
	static BOXED_TPL: Lazy<CompiledInterpolation> = Lazy::new(|| include_str!("tpl/class/boxed.tpl.rs").compile_interpolation());

	static IMPL_TPL: Lazy<CompiledInterpolation> = Lazy::new(|| include_str!("tpl/class/impl.tpl.rs").compile_interpolation());

	static IMPL_EXPLICIT_CLONE_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/impl_explicit_clone.tpl.rs").compile_interpolation());

	static IMPL_IMPLICIT_CLONE_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/impl_implicit_clone.tpl.rs").compile_interpolation());

	static IMPL_DEFAULT_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/impl_default.tpl.rs").compile_interpolation());

	static IMPL_DEBUG_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/impl_debug.rs").compile_interpolation());

	static DEFAULT_CTOR: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/default_ctor.tpl.rs").compile_interpolation());

	static SIMPLE_TPL: Lazy<CompiledInterpolation> = Lazy::new(|| include_str!("tpl/class/simple.tpl.rs").compile_interpolation());

	static SIMPLE_FIELD_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/simple_field.tpl.rs").compile_interpolation());

	static BASE_TPL: Lazy<CompiledInterpolation> = Lazy::new(|| include_str!("tpl/class/base.tpl.rs").compile_interpolation());

	static BASE_CAST_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/base_cast.tpl.rs").compile_interpolation());

	static DESCENDANT_CAST_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/descendant_cast.tpl.rs").compile_interpolation());

	static SIMPLE_BASE_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/simple_base.tpl.rs").compile_interpolation());

	static TRAIT_TPL: Lazy<CompiledInterpolation> = Lazy::new(|| include_str!("tpl/class/trait.tpl.rs").compile_interpolation());

	let type_ref = c.type_ref();
	let is_trait = c.is_trait();
	let class_kind = c.kind();
	let doc_comment = c.rendered_doc_comment(opencv_version);

	let mut out = String::new();

	let consts = c.consts();
	let fields = c.fields();
	let (mut const_methods, mut mut_methods) = if class_kind.is_simple() {
		(vec![], vec![])
	} else {
		(
			c.field_methods(
				fields.iter().filter(|f| f.exclude_kind().is_included()),
				Some(Constness::Const),
			),
			c.field_methods(fields.iter().filter(|f| f.exclude_kind().is_included()), Some(Constness::Mut)),
		)
	};
	let mut field_const_methods = const_methods.clone();
	const_methods.extend(c.methods(Some(Constness::Const)));
	mut_methods.extend(c.methods(Some(Constness::Mut)));
	let method_count = const_methods.len() + mut_methods.len();
	if is_trait {
		let bases = c.bases();
		let mut bases_const = Vec::with_capacity(bases.len());
		let mut bases_mut = Vec::with_capacity(bases.len() + 1);
		bases_mut.push(c.rust_trait_name(NameStyle::ref_(), Constness::Const).into_owned());
		// todo, allow extension of simple classes for e.g. Elliptic_KeyPoint
		for b in bases
			.iter()
			.filter(|b| b.exclude_kind().is_included() && !b.kind().is_simple())
		{
			bases_const.push(b.rust_trait_name(NameStyle::ref_(), Constness::Const).into_owned());
			bases_mut.push(b.rust_trait_name(NameStyle::ref_(), Constness::Mut).into_owned());
		}
		bases_const.sort_unstable();
		bases_mut.sort_unstable();
		let mut trait_bases_const: String = bases_const.join(" + ");
		if !trait_bases_const.is_empty() {
			trait_bases_const.insert_str(0, ": ");
		};
		let mut trait_bases_mut: String = bases_mut.join(" + ");
		if !trait_bases_mut.is_empty() {
			trait_bases_mut.insert_str(0, ": ");
		};
		let mut trait_methods_pool = NamePool::with_capacity(method_count);
		let trait_const_methods = rust_generate_funcs(
			const_methods.iter().filter(|m| m.kind().as_instance_method().is_some()),
			&mut trait_methods_pool,
			opencv_version,
		);
		let trait_mut_methods = rust_generate_funcs(
			mut_methods.iter().filter(|m| m.kind().as_instance_method().is_some()),
			&mut trait_methods_pool,
			opencv_version,
		);

		let rust_local = type_ref.rust_name(NameStyle::ref_());
		let const_trait_comment = format!("/// Constant methods for [{rust_local}]").into();
		let mut_trait_comment = format!("/// Mutable methods for [{rust_local}]").into();

		out = TRAIT_TPL.interpolate(&HashMap::from([
			("const_trait_comment", const_trait_comment),
			("mut_trait_comment", mut_trait_comment),
			("debug", c.get_debug().into()),
			("rust_trait_local", c.rust_trait_name(NameStyle::decl(), Constness::Mut)),
			(
				"rust_trait_local_const",
				c.rust_trait_name(NameStyle::decl(), Constness::Const),
			),
			("rust_local", type_ref.rust_name(NameStyle::decl())),
			(
				"rust_extern_const",
				type_ref.with_constness(Constness::Const).rust_extern(ExternDir::ToCpp),
			),
			(
				"rust_extern_mut",
				type_ref.with_constness(Constness::Mut).rust_extern(ExternDir::ToCpp),
			),
			("trait_bases_const", trait_bases_const.into()),
			("trait_bases_mut", trait_bases_mut.into()),
			("trait_const_methods", trait_const_methods.into()),
			("trait_mut_methods", trait_mut_methods.into()),
		]));
	}

	let rust_local = c.rust_name(NameStyle::decl());
	let mut impls = if c.has_explicit_clone() {
		IMPL_EXPLICIT_CLONE_TPL.interpolate(&HashMap::from([("rust_local", rust_local.as_ref())]))
	} else if c.has_implicit_clone() {
		IMPL_IMPLICIT_CLONE_TPL.interpolate(&HashMap::from([("rust_local", rust_local.as_ref())]))
	} else {
		"".to_string()
	};

	let mut bases = all_bases(c);
	if class_kind.is_boxed() {
		if c.is_polymorphic() {
			let descendants = descendants(c);
			for d in descendants {
				let desc_local = d.rust_name(NameStyle::decl());
				let desc_full = d.rust_name(NameStyle::ref_());
				impls += &DESCENDANT_CAST_TPL.interpolate(&HashMap::from([
					("rust_local", rust_local.as_ref()),
					("descendant_rust_local", desc_local.as_ref()),
					("descendant_rust_full", desc_full.as_ref()),
				]));
			}
		}
		for b in &bases {
			let base_local = b.rust_name(NameStyle::decl());
			let base_full = b.rust_name(NameStyle::ref_());
			impls += &BASE_CAST_TPL.interpolate(&HashMap::from([
				("rust_local", rust_local.as_ref()),
				("base_rust_local", base_local.as_ref()),
				("base_rust_full", base_full.as_ref()),
			]));
		}
		if !settings::IMPLEMENTED_MANUAL_DEBUG.contains(c.cpp_name(CppNameStyle::Reference).as_ref()) {
			for b in &bases {
				let base_fields = b.fields();
				let base_field_const_methods = b.field_methods(
					base_fields.iter().filter(|f| f.exclude_kind().is_included()),
					Some(Constness::Const),
				);
				field_const_methods.extend(base_field_const_methods);
			}
			let debug_fields = rust_generate_debug_fields(field_const_methods);
			impls += &IMPL_DEBUG_TPL.interpolate(&HashMap::from([
				("rust_local", rust_local.as_ref()),
				("debug_fields", &debug_fields),
			]));
		}
	}

	if is_trait {
		bases.push(c.clone());
	}
	let bases = bases
		.into_iter()
		.map(|base| {
			let base_type_ref = base.type_ref();
			let tpl = if class_kind.is_simple() {
				&SIMPLE_BASE_TPL
			} else {
				&BASE_TPL
			};
			tpl.interpolate(&HashMap::from([
				("base_rust_full", base.rust_trait_name(NameStyle::ref_(), Constness::Mut)),
				(
					"base_const_rust_full",
					base.rust_trait_name(NameStyle::ref_(), Constness::Const),
				),
				("rust_local", type_ref.rust_name(NameStyle::decl())),
				("base_rust_local", base_type_ref.rust_name(NameStyle::decl())),
				(
					"base_rust_extern_const",
					base_type_ref.with_constness(Constness::Const).rust_extern(ExternDir::ToCpp),
				),
				(
					"base_rust_extern_mut",
					base_type_ref.with_constness(Constness::Mut).rust_extern(ExternDir::ToCpp),
				),
			]))
		})
		.collect::<Vec<_>>();

	let fields = if class_kind.is_simple() {
		fields
			.into_iter()
			.map(|f| {
				let type_ref = f.type_ref();
				let mut typ = type_ref.rust_name(NameStyle::ref_());
				// hack for converting the references to array types in struct definitions
				if type_ref.as_fixed_array().is_some() {
					if let Some(new_typ) = typ.strip_prefix("&mut ") {
						typ = new_typ.to_string().into()
					}
				}
				SIMPLE_FIELD_TPL.interpolate(&HashMap::from([
					("doc_comment", Cow::Owned(f.rendered_doc_comment(opencv_version))),
					("visibility", "pub ".into()),
					("name", f.rust_leafname(FishStyle::No)),
					("type", typ),
				]))
			})
			.collect()
	} else {
		vec![]
	};

	let mut inherent_methods = String::with_capacity(512 * (const_methods.len() + mut_methods.len()));
	let mut inherent_methods_pool = NamePool::with_capacity(method_count);

	let mut needs_default_impl = false;
	if let Some(def_cons) = mut_methods
		.iter()
		.find(|m| m.is_default_constructor() && m.exclude_kind().is_included())
	{
		if def_cons.return_kind().is_infallible() {
			needs_default_impl = true;
		}
	}
	let needs_default_ctor = needs_default_ctor(class_kind, c, const_methods.iter().chain(mut_methods.iter()));
	if needs_default_ctor {
		inherent_methods.push_str(&DEFAULT_CTOR.interpolate(&HashMap::from([("rust_local", rust_local.as_ref())])));
		inherent_methods_pool.add_name("default");
		needs_default_impl = true;
	}

	if needs_default_impl {
		impls += &IMPL_DEFAULT_TPL.interpolate(&HashMap::from([("rust_local", rust_local.as_ref())]));
	}

	inherent_methods.push_str(&if is_trait {
		rust_generate_funcs(
			const_methods.iter().chain(mut_methods.iter()).filter(|m| {
				let kind = m.kind();
				kind.as_static_method().is_some() || kind.as_constructor().is_some()
			}),
			&mut inherent_methods_pool,
			opencv_version,
		)
	} else {
		rust_generate_funcs(
			const_methods.iter().chain(mut_methods.iter()),
			&mut inherent_methods_pool,
			opencv_version,
		)
	});

	let tpl = if class_kind.is_simple() {
		&SIMPLE_TPL
	} else {
		&BOXED_TPL
	};

	let consts = consts.iter().map(|c| c.gen_rust(opencv_version)).join("");

	out += &tpl.interpolate(&HashMap::from([
		("doc_comment", Cow::Owned(doc_comment)),
		("debug", c.get_debug().into()),
		("rust_local", rust_local.clone()),
		("rust_full", c.rust_name(NameStyle::ref_())),
		(
			"rust_extern_const",
			type_ref.with_constness(Constness::Const).rust_extern(ExternDir::ToCpp),
		),
		(
			"rust_extern_mut",
			type_ref.with_constness(Constness::Mut).rust_extern(ExternDir::ToCpp),
		),
		("fields", fields.join("").into()),
		("bases", bases.join("").into()),
		(
			"impl",
			IMPL_TPL
				.interpolate(&HashMap::from([
					("rust_local", rust_local),
					("consts", consts.into()),
					("inherent_methods", inherent_methods.into()),
				]))
				.into(),
		),
		("impls", impls.into()),
	]));
	out
}

fn gen_rust_exports_boxed(c: &Class) -> String {
	let fields = c.fields();
	let mut out = String::with_capacity((fields.len() + 1) * 128);
	for func in c.field_methods(fields.iter().filter(|f| f.exclude_kind().is_included()), None) {
		if func.exclude_kind().is_included() {
			out += &func.gen_rust_exports();
		}
	}
	out
}

fn gen_cpp_boxed(c: &Class) -> String {
	static BOXED_CPP_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/boxed.tpl.cpp").compile_interpolation());

	static DESCENDANT_CAST_TPL: Lazy<CompiledInterpolation> =
		Lazy::new(|| include_str!("tpl/class/descendant_cast.tpl.cpp").compile_interpolation());

	let fields = c.fields();
	let mut out = String::with_capacity(fields.len() * 512);
	for func in c.field_methods(fields.iter().filter(|f| f.exclude_kind().is_included()), None) {
		if func.exclude_kind().is_included() {
			out += &func.gen_cpp();
		}
	}

	let mut casts = String::new();
	let rust_local = c.rust_name(NameStyle::decl());
	let bases = all_bases(c);
	if c.kind().is_boxed() {
		if c.is_polymorphic() {
			let cpp_decl = c.type_ref().cpp_self_func_decl(Constness::Mut);
			for d in descendants(c) {
				let desc_rust_local = d.rust_name(NameStyle::decl());
				let desc_cpp_ref = d.cpp_name(CppNameStyle::Reference);
				casts += &DESCENDANT_CAST_TPL.interpolate(&HashMap::from([
					("rust_local", rust_local.as_ref()),
					("descendant_rust_local", desc_rust_local.as_ref()),
					("descendant_cpp_full", desc_cpp_ref.as_ref()),
					("cpp_decl", &cpp_decl),
				]));
			}
		}
		for b in bases {
			casts += &method_cast_to_base(c.clone(), b, &rust_local).gen_cpp();
		}
	}

	let type_ref = c.type_ref();
	let delete = FuncDesc::method_delete(&rust_local, c.clone()).gen_cpp();
	out += &BOXED_CPP_TPL.interpolate(&HashMap::from([
		("rust_local", type_ref.rust_name(NameStyle::decl())),
		("cpp_full", type_ref.cpp_name(CppNameStyle::Reference)),
		("cpp_extern", type_ref.cpp_extern()),
		("casts", casts.into()),
		("delete", delete.into()),
	]));
	out
}

fn rust_generate_funcs<'f, 'tu, 'ge>(
	fns: impl Iterator<Item = &'f Func<'tu, 'ge>>,
	name_pool: &mut NamePool,
	opencv_version: &str,
) -> String
where
	'tu: 'ge,
	'ge: 'f,
{
	let fns = fns.filter(|f| f.exclude_kind().is_included());
	fns.map(move |func| {
		let mut func = Cow::Borrowed(func);
		let mut name = func.rust_leafname(FishStyle::No);
		if name_pool.make_unique_name(&mut name).is_changed() {
			let name = name.into();
			func.to_mut().set_custom_rust_leafname(Some(name));
		}
		func.gen_rust(opencv_version) // fixme
	})
	.join("")
}

pub fn rust_generate_debug_fields(field_const_methods: Vec<Func>) -> String {
	field_const_methods
		.into_iter()
		.filter(|f| f.exclude_kind().is_included() && f.return_type_ref().is_debug())
		.filter_map(|f| {
			f.kind().as_field_accessor().map(|(cls, _)| {
				format!(
					"\n\t.field(\"{name}\", &{trait_name}::{name}(self))",
					trait_name = cls.rust_trait_name(NameStyle::ref_fish(), Constness::Const),
					name = f.rust_leafname(FishStyle::No)
				)
			})
		})
		.join("")
}

impl RustElement for Class<'_, '_> {
	fn rust_module(&self) -> Cow<str> {
		match self {
			&Self::Clang { entity, .. } => DefaultRustNativeElement::rust_module(entity),
			Self::Desc(desc) => desc.rust_fullname.module().into(),
		}
	}

	fn rust_name(&self, style: NameStyle) -> Cow<str> {
		match self {
			&Self::Clang { entity, .. } => DefaultRustNativeElement::rust_name(self, entity, style).into(),
			Self::Desc(desc) => match style {
				NameStyle::Declaration => desc.rust_fullname.localname().into(),
				NameStyle::Reference(fish) => fish.apply(desc.rust_fullname.as_ref()),
			},
		}
	}

	fn rust_leafname(&self, _fish_style: FishStyle) -> Cow<str> {
		if self.string_type().is_some() {
			"String".into()
		} else {
			let cpp_declname = self.cpp_name(CppNameStyle::Declaration);
			if cpp_declname == "Vec" {
				"VecN".into()
			} else {
				cpp_declname
			}
		}
	}

	fn rendered_doc_comment_with_prefix(&self, prefix: &str, opencv_version: &str) -> String {
		match self {
			&Self::Clang { entity, .. } => {
				DefaultRustNativeElement::rendered_doc_comment_with_prefix(entity, prefix, opencv_version)
			}
			Self::Desc(_) => "".to_string(),
		}
	}
}

impl RustNativeGeneratedElement for Class<'_, '_> {
	fn element_safe_id(&self) -> String {
		format!("{}-{}", self.rust_module(), self.rust_name(NameStyle::decl()))
	}

	fn gen_rust(&self, opencv_version: &str) -> String {
		match self.kind() {
			ClassKind::Simple | ClassKind::Boxed | ClassKind::BoxedForced => gen_rust_class(self, opencv_version),
			ClassKind::System | ClassKind::Other => "".to_string(),
		}
	}

	fn gen_rust_exports(&self) -> String {
		let out = match self.kind() {
			ClassKind::Boxed | ClassKind::BoxedForced => gen_rust_exports_boxed(self),
			ClassKind::Simple | ClassKind::System | ClassKind::Other => "".to_string(),
		};

		let mut methods = self
			.methods(None)
			.into_iter()
			.filter(|m| m.exclude_kind().is_included())
			.map(|m| m.gen_rust_exports());

		out + &methods.join("")
	}

	fn gen_cpp(&self) -> String {
		let out = match self.kind() {
			ClassKind::Boxed | ClassKind::BoxedForced => gen_cpp_boxed(self),
			ClassKind::Simple | ClassKind::System | ClassKind::Other => "".to_string(),
		};

		let methods = self.methods(None);

		let needs_default_ctor = needs_default_ctor(self.kind(), self, methods.iter());

		let mut cpp_methods: Vec<_> = methods
			.into_iter()
			.filter(|m| m.exclude_kind().is_included())
			.map(|m| m.gen_cpp())
			.collect();

		let has_implicit_clone = self.has_implicit_clone();

		if has_implicit_clone || needs_default_ctor {
			let rust_local = self.rust_name(NameStyle::decl());
			let type_ref = self.type_ref();
			if has_implicit_clone {
				cpp_methods.push(method_implicit_clone(&rust_local, type_ref.clone()).gen_cpp());
			}
			if needs_default_ctor {
				cpp_methods.push(method_default_new(&rust_local, type_ref).gen_cpp());
			}
		}

		out + &cpp_methods.join("")
	}
}

fn needs_default_ctor<'r>(kind: ClassKind, c: &Class, mut methods: impl Iterator<Item = &'r Func<'r, 'r>>) -> bool {
	matches!(kind, ClassKind::BoxedForced)
		&& !c.is_abstract()
		&& methods.all(|m| !m.kind().as_constructor().is_some() || m.exclude_kind().is_excluded())
}

fn all_bases<'tu, 'ge>(cls: &Class<'tu, 'ge>) -> Vec<Class<'tu, 'ge>> {
	let mut out = cls.all_bases().into_iter()
		.filter(|b| b.exclude_kind().is_included() && !b.kind().is_simple()) // todo, allow extension of simple classes for e.g. Elliptic_KeyPoint
		.collect::<Vec<_>>();
	out.sort_unstable_by(|a, b| {
		a.cpp_name(CppNameStyle::Declaration)
			.cmp(&b.cpp_name(CppNameStyle::Declaration))
	});
	out
}

fn descendants<'tu, 'ge>(cls: &Class<'tu, 'ge>) -> Vec<Class<'tu, 'ge>> {
	let mut out = cls
		.descendants()
		.into_iter()
		.filter(|d| d.exclude_kind().is_included() && !d.kind().is_simple())
		.collect::<Vec<_>>();
	out.sort_unstable_by(|a, b| {
		a.cpp_name(CppNameStyle::Declaration)
			.cmp(&b.cpp_name(CppNameStyle::Declaration))
	});
	out
}

fn method_default_new<'tu, 'ge>(rust_local: &str, type_ref: TypeRef<'tu, 'ge>) -> Func<'tu, 'ge> {
	Func::new_desc(FuncDesc::new(
		FuncKind::Function,
		Constness::Const,
		ReturnKind::InfallibleNaked,
		format!("cv::{rust_local}::defaultNew"),
		"<unused>",
		vec![],
		FuncCppBody::ManualFull("return new {{ret_type}}();".into()),
		type_ref,
	))
}

fn method_implicit_clone<'tu, 'ge>(rust_local: &str, type_ref: TypeRef<'tu, 'ge>) -> Func<'tu, 'ge> {
	Func::new_desc(FuncDesc::new(
		FuncKind::Function,
		Constness::Const,
		ReturnKind::InfallibleNaked,
		format!("cv::{rust_local}::implicitClone"),
		"<unused>",
		vec![Field::new_desc(FieldDesc::new("val", type_ref.clone()))],
		FuncCppBody::ManualFull(format!("return {};", cpp_return_map(&type_ref, "*val", false).0).into()),
		type_ref,
	))
}

fn method_cast_to_base<'tu, 'ge>(class: Class<'tu, 'ge>, base_class: Class<'tu, 'ge>, rust_local: &str) -> Func<'tu, 'ge> {
	let base_rust_local = base_class.rust_name(NameStyle::decl());
	Func::new_desc(FuncDesc::new(
		FuncKind::InstanceMethod(class),
		Constness::Mut,
		ReturnKind::InfallibleNaked,
		format!("cv::{rust_local}::to_{base_rust_local}"),
		"<unused>",
		vec![],
		FuncCppBody::ManualFull("return dynamic_cast<{{ret_type}}*>(instance);".into()),
		TypeRef::new_desc(TypeRefDesc::new(TypeRefKind::Class(base_class))),
	))
}

pub trait ClassExt {
	fn rust_trait_name(&self, style: NameStyle, constness: Constness) -> Cow<str>;
}

impl ClassExt for Class<'_, '_> {
	fn rust_trait_name(&self, style: NameStyle, constness: Constness) -> Cow<str> {
		let mut out = self.rust_name(style);
		if self.is_trait() {
			if constness.is_const() {
				out.to_mut().push_str("TraitConst");
			} else {
				out.to_mut().push_str("Trait");
			}
		}
		out
	}
}
